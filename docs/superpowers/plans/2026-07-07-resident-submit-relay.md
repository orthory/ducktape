# Resident Submit Relay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resident-standing nodes can author ops (chat, kv, …) signed with their own identity key, relayed over a new mesh channel to a validator that takes consensus custody.

**Architecture:** A new static discovery channel (`CHANNEL_SUBMIT_RELAY = 3`) carries resident-signed frame bytes to a current validator. The kernel gains `OrderedNode::submit_frame` (custody of an already-signed frame). The validator gates relays on committed resident standing + origin==peer, injects, and answers with the frame's consensus fate when it drains. The resident's park-loop serve window accepts writes when standing, holds the caller's reply until the relay answer.

**Tech Stack:** Rust; commonware p2p authenticated discovery; serde_json wire (lobby idiom); existing e2e harness `NetworkShapeCluster`.

**Spec:** `docs/superpowers/specs/2026-07-07-resident-submit-relay-design.md`

## Global Constraints

- Branch `feat/observer-submit-relay`, base `origin/dev`; PR targets `dev` (never `main`).
- Commit with `git -c commit.gpgsign=false commit …` (SSH signing hangs in this env).
- `MAX_PROTOCOL_VERSION` stays 3 — the relay is node-binary capability, not consensus semantics.
- An unregistered channel is a protocol violation that kills the sender's connection: `CHANNEL_SUBMIT_RELAY` must be registered in EVERY mode (validator serves; joiner/resident client; sync-only black-holes).
- Comment style: match main.rs — dense, WHY-focused, lowercase-leading.
- Line numbers below are from `origin/dev` @ cc831cc — re-anchor with the quoted context if drifted.

---

### Task 1: Kernel `OrderedNode::submit_frame`

**Files:**
- Modify: `crates/kernel/node/src/lib.rs` (`submit` at ~line 956)
- Test: `crates/kernel/node/tests/submit_frame.rs` (new)

**Interfaces:**
- Produces: `pub async fn submit_frame(&mut self, frame: Vec<u8>) -> Result<FrameId, Error>` on `OrderedNode<O>`; existing `pub fn encode_frame(signer, seq, &Msg) -> Vec<u8>`, `pub fn frame_id(bytes) -> FrameId`, `pub fn decode_frame(bytes) -> Result<(Origin, Msg), Error>` are already public and unchanged.

- [ ] **Step 1: Write the failing test**

`crates/kernel/node/tests/submit_frame.rs` (harness mirrors `epoch_cutover_gate.rs`):

```rust
//! `submit_frame` — custody of an ALREADY-SIGNED frame: the relay entry
//! point. verification precedes pinning (junk never enters custody), and a
//! relayed frame's authorship is the SIGNER's key, not the submitting node's.

use directory::Directory;
use directory::{DirMsg, DirQuery, DirReply, decode_reply, encode_msg, encode_query};
use futures::executor::block_on;
use host::Host;
use node::{OrderedNode, RoundOrderer};
use sdk::Msg;

fn sk(seed: u64) -> commonware_cryptography::ed25519::PrivateKey {
    use commonware_cryptography::Signer as _;
    commonware_cryptography::ed25519::PrivateKey::from_seed(seed)
}

fn set(key: &str, value: &str) -> Msg {
    Msg {
        target: "directory".into(),
        payload: encode_msg(&DirMsg::Set {
            key: key.into(),
            value: value.into(),
        }),
    }
}

async fn get(node: &OrderedNode<RoundOrderer>, key: &str) -> Option<String> {
    let reply = node
        .host()
        .query(
            "directory",
            &encode_query(&DirQuery::Get { key: key.into() }),
        )
        .await
        .expect("query");
    match decode_reply(&reply).expect("decode") {
        DirReply::Value(v) => v,
    }
}

#[test]
fn submit_frame_takes_custody_and_keeps_signer_authorship() {
    block_on(async {
        use commonware_cryptography::Signer as _;
        let host = Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis");
        let mut node = OrderedNode::new(host, RoundOrderer::new());

        // a frame signed by a key that is NOT this node's submitter.
        let author = sk(7);
        let frame = node::encode_frame(&author, 0, &set("k", "v"));
        let expected_id = node::frame_id(&frame);

        let id = node.submit_frame(frame).await.expect("submit_frame");
        assert_eq!(id, expected_id, "the returned id is the frame's content address");

        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        assert_eq!(get(&node, "k").await.as_deref(), Some("v"));

        // authorship is the SIGNER's key — what modules read as Env.origin.
        let drained = node.take_drained();
        let d = drained.iter().find(|d| d.id == id).expect("drained frame");
        match &d.op {
            Some(op) => assert_eq!(
                op.origin,
                sdk::Origin::External(author.public_key().as_ref().to_vec()),
                "authorship rides the signature, not the custodian",
            ),
            None => panic!("applied frame carries its decoded op"),
        }
    });
}

#[test]
fn submit_frame_rejects_tampered_bytes_before_custody() {
    block_on(async {
        let host = Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis");
        let mut node = OrderedNode::new(host, RoundOrderer::new());

        let mut frame = node::encode_frame(&sk(7), 0, &set("k", "v"));
        // flip one payload byte: the signature no longer binds.
        let last = frame.len() - 1;
        frame[last] ^= 0x01;

        assert!(
            node.submit_frame(frame).await.is_err(),
            "a frame whose signature does not verify is refused at the door"
        );
        // nothing was proposed: the next drain delivers no frames.
        assert_eq!(node.drain_delivered().await.expect("drain"), 0);
        assert_eq!(get(&node, "k").await, None);
    });
}

#[test]
fn submit_still_equals_sign_plus_submit_frame() {
    block_on(async {
        use commonware_cryptography::Signer as _;
        let host = Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis");
        let mut node = OrderedNode::new(host, RoundOrderer::new());

        let signer = sk(1);
        let via_submit = node.submit(&signer, 0, set("a", "1")).await.expect("submit");
        let by_hand = node::frame_id(&node::encode_frame(&signer, 0, &set("a", "1")));
        assert_eq!(via_submit, by_hand, "submit is sign + submit_frame, byte-identical");
    });
}
```

Note: if `Drained`'s op field/origin shape differs (check the struct near line 600, `pub origin: Origin` on the drained record vs inside `op`), adjust the authorship assert to the actual field — the intent is: the drained record's authenticated origin equals `External(author pubkey bytes)`.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p node --test submit_frame`
Expected: compile FAIL — `submit_frame` not found.

- [ ] **Step 3: Implement**

In `crates/kernel/node/src/lib.rs`, replace the body of `submit` and add `submit_frame` right below it:

```rust
    pub async fn submit(
        &mut self,
        signer: &PrivateKey,
        seq: u64,
        msg: Msg,
    ) -> Result<FrameId, Error> {
        let frame = encode_frame(signer, seq, &msg);
        self.submit_frame(frame).await
    }

    /// take custody of an ALREADY-SIGNED frame (the relay entry point: an
    /// resident signs with its own identity key, a validator injects). the
    /// signature is verified BEFORE anything is pinned — junk from the wire
    /// must never enter the durable store or the orderer. custody semantics
    /// are identical to [`OrderedNode::submit`]: pin, propose, track
    /// outstanding (the cutover carry and the exactly-once digest gate treat
    /// a relayed frame exactly like a local one).
    pub async fn submit_frame(&mut self, frame: Vec<u8>) -> Result<FrameId, Error> {
        decode_frame(&frame)?;
        let id = frame_id(&frame);
        // durably pin the bytes BEFORE the orderer may propose their digest:
        // once the engine journals a finalization, these bytes are the only
        // thing standing between a crash and an unrecoverable finalized op
        // (the content store is memory; the engine journals votes, not
        // payloads; a solo network has no peer to refetch from).
        self.sink.pin(&frame).await?;
        self.orderer.submit(frame.clone()).await?;
        // custody begins only on FULL acceptance (pinned + proposed): an
        // errored submit is reported to the caller, who retries — tracking
        // it would double the op when the retry lands and a cutover carries
        // the failed original too.
        let (_, seq) = frame_origin_seq(&frame).expect("decode_frame verified the envelope");
        self.outstanding.insert(id, (seq, frame));
        Ok(id)
    }
```

(The old `submit` body's pin/propose/track block moves into `submit_frame`; `submit` keeps only sign + delegate. `frame_origin_seq` already exists and is infallible after `decode_frame` passed.)

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p node --test submit_frame`
Expected: 3 passed.

Also run the neighbors that exercise custody: `cargo test -p node`
Expected: all pass (cutover carry unchanged).

- [ ] **Step 5: Commit**

```bash
git add crates/kernel/node
git -c commit.gpgsign=false commit -m "feat(kernel): OrderedNode::submit_frame — custody of a pre-signed frame

verification precedes pinning; submit is now sign + submit_frame.
the relay entry point: authorship rides the signature, custody the
injecting node.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: Relay wire module `bin/node/src/relay.rs`

**Files:**
- Create: `bin/node/src/relay.rs`
- Modify: `bin/node/src/main.rs` (add `mod relay;` next to `mod lobby;` at ~line 70)

**Interfaces:**
- Consumes: `node::{decode_frame, frame_id, FrameId}`, `sdk::Origin`.
- Produces:
  - `pub enum RelayMsg { Submit { frame: Vec<u8> }, Reply { frame_id: [u8; 32], outcome: RelayOutcome } }`
  - `pub enum RelayOutcome { Applied { height: u64, app_hash: String }, Rejected { detail: String }, Refused { detail: String } }`
  - `pub fn encode_msg(&RelayMsg) -> Vec<u8>`, `pub fn decode_msg(&[u8]) -> Result<RelayMsg, String>`
  - `pub fn verify_relay_submit(frame: &[u8], sender: &[u8], residents: &[Vec<u8>]) -> Result<node::FrameId, String>`

- [ ] **Step 1: Write the module with failing tests inline**

```rust
//! the submit-relay channel wire format — how a resident-standing node
//! delivers a frame it signed, and how a validator answers with the frame's
//! consensus fate.
//!
//! transport: the resident is already an authenticated mesh peer; it speaks
//! on `CHANNEL_SUBMIT_RELAY` to ONE current validator. the message carries
//! the frame bytes exactly as `node::encode_frame` produced them — the
//! frame's own signature (origin, seq, target, payload) is the authorship;
//! the channel peer identity only GATES (origin must equal the sender, so a
//! node relays nothing but its own ops, and the origin must hold committed
//! resident standing). the validator takes consensus custody via
//! `submit_frame` and replies when the frame drains — Applied with the
//! sealed block's coordinates, Rejected for a deterministic no-op, Refused
//! for door failures and expired holds.
//!
//! json on the wire: matches the lobby idiom — this lane is low-volume (a
//! human posting messages), and the frame bytes inside are already signed.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RelayOutcome {
    /// drained Applied at `height`; `app_hash` is the PER-BLOCK boundary
    /// hash the frame settled at (what a local app-surface hold reports).
    Applied { height: u64, app_hash: String },
    /// finalized but deterministically rejected by its module.
    Rejected { detail: String },
    /// refused at the door (bad frame / origin mismatch / no standing) or
    /// the validator's hold expired before finalization — the op may still
    /// land later; clients re-query on block events.
    Refused { detail: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RelayMsg {
    /// a resident-signed frame, bytes exactly as `encode_frame` produced.
    Submit { frame: Vec<u8> },
    /// the validator's answer, keyed by the frame's content address.
    Reply {
        frame_id: [u8; 32],
        outcome: RelayOutcome,
    },
}

pub fn encode_msg(m: &RelayMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}

pub fn decode_msg(b: &[u8]) -> Result<RelayMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

/// the validator's door check, pure so it is testable without a mesh:
/// the frame must decode AND verify (the kernel checks the signature binds
/// origin/seq/target/payload), its origin must BE the sending peer (a node
/// relays only its own ops — no laundering), and that origin must hold
/// committed resident standing (validators submit locally; parked joiners
/// have no standing). membership-current state is the CALLER's to fetch —
/// this needs only bytes.
pub fn verify_relay_submit(
    frame: &[u8],
    sender: &[u8],
    residents: &[Vec<u8>],
) -> Result<node::FrameId, String> {
    let (origin, _msg) = node::decode_frame(frame).map_err(|e| format!("bad frame: {e}"))?;
    let sdk::Origin::External(origin_bytes) = origin else {
        return Err("relayed frames carry an external origin".into());
    };
    if origin_bytes.as_slice() != sender {
        return Err("frame origin is not the relaying peer — a node relays only its own ops".into());
    }
    if !residents.iter().any(|o| o.as_slice() == sender) {
        return Err("origin holds no committed resident standing — submit via a validator".into());
    }
    Ok(node::frame_id(frame))
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::Signer as _;

    fn sk(seed: u64) -> commonware_cryptography::ed25519::PrivateKey {
        commonware_cryptography::ed25519::PrivateKey::from_seed(seed)
    }

    fn msg() -> sdk::Msg {
        sdk::Msg {
            target: "kv".into(),
            payload: b"{}".to_vec(),
        }
    }

    #[test]
    fn wire_round_trips() {
        for m in [
            RelayMsg::Submit { frame: vec![1, 2, 3] },
            RelayMsg::Reply {
                frame_id: [7; 32],
                outcome: RelayOutcome::Applied {
                    height: 42,
                    app_hash: "aa".into(),
                },
            },
            RelayMsg::Reply {
                frame_id: [0; 32],
                outcome: RelayOutcome::Refused { detail: "x".into() },
            },
        ] {
            assert_eq!(decode_msg(&encode_msg(&m)).expect("round trip"), m);
        }
    }

    #[test]
    fn door_accepts_a_standing_residents_own_frame() {
        let author = sk(7);
        let me = author.public_key().as_ref().to_vec();
        let frame = node::encode_frame(&author, 3, &msg());
        let id = verify_relay_submit(&frame, &me, &[me.clone()]).expect("accepted");
        assert_eq!(id, node::frame_id(&frame));
    }

    #[test]
    fn door_refuses_origin_that_is_not_the_sender() {
        let author = sk(7);
        let other = sk(8).public_key().as_ref().to_vec();
        let frame = node::encode_frame(&author, 0, &msg());
        let err = verify_relay_submit(&frame, &other, &[other.clone()]).unwrap_err();
        assert!(err.contains("only its own ops"), "{err}");
    }

    #[test]
    fn door_refuses_without_resident_standing() {
        let author = sk(7);
        let me = author.public_key().as_ref().to_vec();
        let frame = node::encode_frame(&author, 0, &msg());
        let err = verify_relay_submit(&frame, &me, &[]).unwrap_err();
        assert!(err.contains("standing"), "{err}");
    }

    #[test]
    fn door_refuses_tampered_bytes() {
        let author = sk(7);
        let me = author.public_key().as_ref().to_vec();
        let mut frame = node::encode_frame(&author, 0, &msg());
        let last = frame.len() - 1;
        frame[last] ^= 0x01;
        assert!(verify_relay_submit(&frame, &me, &[me.clone()]).is_err());
    }
}
```

Add to `main.rs` next to `mod lobby;`:

```rust
mod relay;
```

- [ ] **Step 2: Run to verify the new tests pass (module is self-contained)**

Run: `cargo test -p ducktape-node relay::`
(if the bin test filter needs it: `cargo test --bin ducktape-node relay`)
Expected: 5 passed. (These are inline unit tests — they pass as soon as the module compiles; the "failing" phase here is the compile error before Step 1 completes, which is expected for a pure codec module.)

- [ ] **Step 3: Commit**

```bash
git add bin/node/src/relay.rs bin/node/src/main.rs
git -c commit.gpgsign=false commit -m "feat(node): submit-relay wire module — codec + pure door check

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: Validator side — serve the relay lane

**Files:**
- Modify: `bin/node/src/main.rs`:
  - constants block (~line 168): add `CHANNEL_SUBMIT_RELAY`
  - validator static registrations (~5175–5188, next to lobby/voice)
  - ingress bridges (~5670–5705, next to the lobby bridge)
  - pump state (~5956, next to `pending_submits`)
  - drain resolution + expiry (~6250–6285)
  - new select arm (next to the lobby arm at ~6960)

**Interfaces:**
- Consumes: `relay::{RelayMsg, RelayOutcome, encode_msg, decode_msg, verify_relay_submit}`, `node.submit_frame`, `read_valset_residents`.
- Produces: a validator that answers `RelayMsg::Submit` with `RelayMsg::Reply` on drain/expiry — what Task 4's resident client consumes.

- [ ] **Step 1: Add the channel constant** (after `CHANNEL_LOBBY`, renumber nothing):

```rust
/// the submit-relay channel: a resident-standing node ships a frame it
/// SIGNED (its own identity key is the frame origin — authorship) to one
/// current validator, which takes consensus custody (`submit_frame`) and
/// answers with the frame's fate when it drains. the last free static slot
/// below CHANNEL_STATE_SYNC; engine banks start at 8. registered in EVERY
/// mode like the lanes above — validators serve, residents speak, sync-only
/// black-holes.
const CHANNEL_SUBMIT_RELAY: u64 = 3;
```

- [ ] **Step 2: Register + bridge on the validator path**

Next to the lobby registration (~5179):

```rust
let (mut relay_tx, relay_rx) = network.register(CHANNEL_SUBMIT_RELAY, quota, MAX_BACKLOG);
```

Next to the lobby bridge (~5691), same bounded drop-on-full pattern (a dropped relay degrades to the client's honest timeout + re-submit):

```rust
let (relay_bridge_tx, mut relay_ingress) =
    futures::channel::mpsc::channel::<(ed25519::PublicKey, Vec<u8>)>(64);
context.child("relay_ingress").spawn(move |_ctx| {
    let mut receiver = relay_rx;
    let mut bridge_tx = relay_bridge_tx;
    async move {
        loop {
            match receiver.recv().await {
                Ok((peer, msg)) => {
                    let bytes: Vec<u8> = msg.into();
                    let _ = bridge_tx.try_send((peer, bytes));
                }
                Err(_) => return,
            }
        }
    }
});
```

- [ ] **Step 3: Pump state** — next to `pending_submits` (~5956):

```rust
// relayed submits held for a wire answer, keyed like pending_submits by
// the frame's content address: resolved by the SAME drain that resolves
// local holds, expired on the same SUBMIT_HOLD budget. the peer is where
// the Reply goes.
let mut pending_relays: std::collections::HashMap<
    node::FrameId,
    (ed25519::PublicKey, std::time::Instant),
> = std::collections::HashMap::new();
```

- [ ] **Step 4: The relay select arm** — next to the lobby arm (~6960):

```rust
relayed = relay_ingress.next() => {
    let Some((peer, bytes)) = relayed else { continue };
    let mut send_reply = |frame_id: node::FrameId, outcome: relay::RelayOutcome| {
        let msg = relay::RelayMsg::Reply { frame_id, outcome };
        let _ = relay_tx.send(
            Recipients::One(peer.clone()),
            IoBuf::from(relay::encode_msg(&msg)),
            false,
        );
    };
    let msg = match relay::decode_msg(&bytes) {
        Ok(m) => m,
        Err(_) => continue, // junk on the doorbell — drop, lobby idiom.
    };
    let relay::RelayMsg::Submit { frame } = msg else {
        continue; // a Reply at a validator is a protocol confusion — drop.
    };
    // the door check needs committed state: the resident projection at
    // this node's latest boundary. origin==peer and signature checks ride
    // inside.
    let residents_now = read_valset_residents(node.host()).await;
    let frame_id = match relay::verify_relay_submit(&frame, peer.as_ref(), &residents_now) {
        Ok(id) => id,
        Err(detail) => {
            send_reply(node::frame_id(&frame), relay::RelayOutcome::Refused { detail });
            continue;
        }
    };
    match node.submit_frame(frame).await {
        Ok(id) => {
            debug_assert_eq!(id, frame_id);
            pending_relays.insert(
                id,
                (peer.clone(), std::time::Instant::now() + SUBMIT_HOLD),
            );
        }
        Err(e) => send_reply(
            frame_id,
            relay::RelayOutcome::Refused { detail: format!("submit failed: {e}") },
        ),
    }
}
```

- [ ] **Step 5: Resolve on drain + expire** — in the `for d in drained` hold-resolution loop (~6250) add, next to the `pending_submits.remove` block (discards continue above, as today):

```rust
if let Some((peer, _)) = pending_relays.remove(&d.id) {
    let outcome = match d.disposition {
        node::Disposition::Applied => relay::RelayOutcome::Applied {
            height: d.height,
            app_hash: hex(&d.app_hash),
        },
        node::Disposition::Rejected => relay::RelayOutcome::Rejected {
            detail: "op finalized but rejected (deterministic no-op)".into(),
        },
        node::Disposition::Discarded => unreachable!("filtered at the loop top"),
    };
    let msg = relay::RelayMsg::Reply { frame_id: d.id, outcome };
    let _ = relay_tx.send(
        Recipients::One(peer),
        IoBuf::from(relay::encode_msg(&msg)),
        false,
    );
}
```

CAUTION: the drained loop at ~6191 `for d in drained` consumes `drained` after the indexing loop borrowed it — the relay resolution goes in the SAME loop as the `pending_submits` resolution.

And next to the `pending_submits` expiry sweep (~6269):

```rust
if !pending_relays.is_empty() {
    let now = std::time::Instant::now();
    let expired: Vec<node::FrameId> = pending_relays
        .iter()
        .filter(|(_, (_, deadline))| *deadline <= now)
        .map(|(k, _)| *k)
        .collect();
    for k in expired {
        if let Some((peer, _)) = pending_relays.remove(&k) {
            let msg = relay::RelayMsg::Reply {
                frame_id: k,
                outcome: relay::RelayOutcome::Refused {
                    detail: "timed out awaiting finalization — re-query on the next block".into(),
                },
            };
            let _ = relay_tx.send(Recipients::One(peer), IoBuf::from(relay::encode_msg(&msg)), false);
        }
    }
}
```

- [ ] **Step 6: Compile**

Run: `cargo check -p ducktape-node`
Expected: clean. (Behavioral verification is Task 5's e2e.)

- [ ] **Step 7: Commit**

```bash
git add bin/node/src/main.rs
git -c commit.gpgsign=false commit -m "feat(node): validators serve the submit-relay lane

door check (signature, origin==peer, committed resident standing),
consensus custody via submit_frame, wire reply on drain or expiry —
the same SUBMIT_HOLD contract as local app-surface holds.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: Resident side — accept writes, relay, hold the reply

**Files:**
- Modify: `bin/node/src/main.rs`:
  - sync-only registrations (~4103–4126): black-hole `CHANNEL_SUBMIT_RELAY`
  - joiner path registrations (~4254–4314): register the client lane
  - serve window (~4437–4560): replace both write refusals; add relay-reply arm + holds
  - park-loop tail (after the tick, before the manifest poll): expiry sweep
- Modify: `docs/superpowers/specs/2026-07-07-resident-submit-relay-design.md`: the resident sweep budget is `SUBMIT_HOLD` (not `+5s`) — the rpc bridge caps at 10s anyway; note it.

**Interfaces:**
- Consumes: Task 3's validator behavior; `relay::*`; `node::{encode_frame, frame_id}`; `announce_targets` (current participants, refreshed by the manifest poll); `storage_for_sync` (the joiner path's storage root, ~line 4205).
- Produces: resident surfaces that accept `RpcRequest::Submit` / `NodeCommand::Submit` when `resident_standing && serving.is_some()`.

- [ ] **Step 1: sync-only black-hole** (next to the voice black-hole at ~4126):

```rust
{
    let (_tx, mut rx) = network.register(CHANNEL_SUBMIT_RELAY, quota, MAX_BACKLOG);
    context
        .child("blackhole_submit_relay")
        .spawn(move |_ctx| async move { while rx.recv().await.is_ok() {} });
}
```

- [ ] **Step 2: joiner-path registration** (next to the lobby registration at ~4314):

```rust
// the submit-relay lane: once resident standing lands, writes leave here —
// this node signs its own frames and a validator takes custody. replies
// (the frame's consensus fate) come back on the same lane.
let (mut relay_tx, mut relay_rx) = network.register(CHANNEL_SUBMIT_RELAY, quota, MAX_BACKLOG);
```

- [ ] **Step 3: serve-window state** (next to `serving` at ~4405):

```rust
// the caller's held reply for a relayed submit, keyed by the frame's
// content address. either surface may hold: the rpc bridge sender or the
// app-surface oneshot. swept on the serve-window tick with the validator's
// own SUBMIT_HOLD budget (the rpc bridge times out at the same 10s — a
// sweep race there reads as a stuck node, same as on a validator).
enum RelayHold {
    Rpc(std::sync::mpsc::Sender<RpcReply>),
    Http(futures::channel::oneshot::Sender<Result<noded::BlockSummary, String>>),
}
let mut pending_relayed: std::collections::HashMap<
    node::FrameId,
    (RelayHold, std::time::Instant),
> = std::collections::HashMap::new();
let mut relay_round = 0usize;
// this origin's next frame seq, persisted so restarts keep climbing (a
// lost file restarts at 0 — kernel-safe: distinct digests both apply;
// per-origin nonces are the documented roadmap item).
let relay_seq_file = storage_for_sync.join("relay-submit-seq");
let mut relay_seq: u64 = std::fs::read_to_string(&relay_seq_file)
    .ok()
    .and_then(|s| s.trim().parse().ok())
    .unwrap_or(0);
```

And a small helper closure next to `not_serving`:

```rust
// relay a caller's op: sign with THIS node's identity (the frame origin —
// chat authorship, status.publicKey), bump+persist the seq FIRST (a crash
// between persist and send costs one seq number, never a reuse), ship to
// one current validator round-robin. Err is immediate (nothing held).
let mut relay_submit = |target: String,
                        payload: Vec<u8>,
                        relay_seq: &mut u64,
                        relay_round: &mut usize,
                        announce_targets: &[ed25519::PublicKey],
                        relay_tx: &mut _|
 -> Result<node::FrameId, String> {
    if announce_targets.is_empty() {
        return Err("no validator known yet — the manifest poll has not landed".into());
    }
    *relay_seq += 1;
    if let Err(e) = std::fs::write(&relay_seq_file, relay_seq.to_string()) {
        return Err(format!("cannot persist the submit seq: {e}"));
    }
    let frame = node::encode_frame(&signer, *relay_seq, &Msg { target, payload });
    let id = node::frame_id(&frame);
    let target_v = announce_targets[*relay_round % announce_targets.len()].clone();
    *relay_round += 1;
    let sent = relay_tx.send(
        Recipients::One(target_v),
        IoBuf::from(relay::encode_msg(&relay::RelayMsg::Submit { frame })),
        false,
    );
    if sent.is_empty() {
        return Err("validator unreachable — retry shortly".into());
    }
    Ok(id)
};
```

(If closure captures fight the borrow checker here, hoist it to a plain `fn` taking every input explicitly — the shape above lists them all. Keep behavior identical.)

- [ ] **Step 4: replace the two write refusals** in the serve window:

rpc (~4450) — the refusal stays for the un-standing / not-yet-serving cases:

```rust
RpcRequest::Submit { target, payload_hex } => {
    if !resident_standing || serving.is_none() {
        RpcReply::err(not_serving(resident_standing))
    } else {
        match unhex(&payload_hex) {
            Ok(payload) => match relay_submit(
                target, payload, &mut relay_seq, &mut relay_round,
                &announce_targets, &mut relay_tx,
            ) {
                Ok(id) => {
                    pending_relayed.insert(
                        id,
                        (RelayHold::Rpc(reply.clone()),
                         std::time::Instant::now() + SUBMIT_HOLD),
                    );
                    continue; // held — answered by the relay Reply or the sweep.
                }
                Err(e) => RpcReply::err(e),
            },
            Err(e) => RpcReply::err(format!("bad payload_hex: {e}")),
        }
    }
}
```

CAUTION: the surrounding code ends with one shared `let _ = reply.send(resp);` — the held case must skip it (the `continue` above skips the tail send; make sure the tail is structured so `continue` is legal, mirroring how `RpcRequest::Shutdown` already early-exits).

http (~4509):

```rust
noded::NodeCommand::Submit { target, payload, origin: _, reply } => {
    if !resident_standing || serving.is_none() {
        let _ = reply.send(Err(not_serving(resident_standing)));
    } else {
        match relay_submit(
            target, payload, &mut relay_seq, &mut relay_round,
            &announce_targets, &mut relay_tx,
        ) {
            Ok(id) => {
                pending_relayed.insert(
                    id,
                    (RelayHold::Http(reply),
                     std::time::Instant::now() + SUBMIT_HOLD),
                );
            }
            Err(e) => { /* reply moved above — restructure: */ }
        }
    }
}
```

CAUTION (borrow shape): `reply` is moved into the hold on success but needed for the error path — bind as `let mut reply_slot = Some(reply);` and `take()` it per path, the codebase does this dance elsewhere; keep it explicit and total.

- [ ] **Step 5: the relay-reply arm** in the `select_biased!` (next to the rpc/http arms):

```rust
answer = relay_rx.recv().fuse() => {
    let Ok((_peer, msg)) = answer else { continue };
    let bytes: Vec<u8> = msg.into();
    let Ok(relay::RelayMsg::Reply { frame_id, outcome }) = relay::decode_msg(&bytes) else {
        continue; // junk or a stray Submit — drop.
    };
    let Some((hold, _)) = pending_relayed.remove(&frame_id) else { continue };
    match (hold, outcome) {
        (RelayHold::Rpc(tx), relay::RelayOutcome::Applied { .. }) => {
            let _ = tx.send(RpcReply::ok());
        }
        (RelayHold::Rpc(tx), relay::RelayOutcome::Rejected { detail })
        | (RelayHold::Rpc(tx), relay::RelayOutcome::Refused { detail }) => {
            let _ = tx.send(RpcReply::err(detail));
        }
        (RelayHold::Http(tx), relay::RelayOutcome::Applied { height, app_hash }) => {
            let _ = tx.send(Ok(noded::BlockSummary { height, app_hash }));
        }
        (RelayHold::Http(tx), relay::RelayOutcome::Rejected { detail })
        | (RelayHold::Http(tx), relay::RelayOutcome::Refused { detail }) => {
            let _ = tx.send(Err(detail));
        }
    }
}
```

CAUTION: the serve window's tick loop is recreated per park-loop pass, and `relay_rx.recv()` must not be dropped mid-delivery the way the pump comment warns — mirror the pump's bridge idiom if needed: spawn a forwarder task ONCE (before the park loop) into a `futures::channel::mpsc` and select on THAT here, exactly like `sync_ingress` (~5670). This is the safe default; do it.

- [ ] **Step 6: the expiry sweep** — after the tick `break` (park-loop tail, before the manifest poll):

```rust
// expire relay holds the mesh never answered. the op may still land —
// the app re-queries on block events, same contract as a validator hold.
if !pending_relayed.is_empty() {
    let now = std::time::Instant::now();
    let expired: Vec<node::FrameId> = pending_relayed
        .iter()
        .filter(|(_, (_, deadline))| *deadline <= now)
        .map(|(k, _)| *k)
        .collect();
    for k in expired {
        if let Some((hold, _)) = pending_relayed.remove(&k) {
            let detail = "timed out awaiting the relay answer — re-query on the next block";
            match hold {
                RelayHold::Rpc(tx) => { let _ = tx.send(RpcReply::err(detail)); }
                RelayHold::Http(tx) => { let _ = tx.send(Err(detail.into())); }
            }
        }
    }
}
```

- [ ] **Step 7: Compile + unit tests still green**

Run: `cargo check -p ducktape-node && cargo test -p ducktape-node relay`
Expected: clean, 5 relay tests pass.

- [ ] **Step 8: Amend the spec** (sweep budget = `SUBMIT_HOLD`; rpc-bridge note), then commit:

```bash
git add bin/node/src/main.rs docs/superpowers/specs/2026-07-07-resident-submit-relay-design.md
git -c commit.gpgsign=false commit -m "feat(node): resident surfaces accept writes via the submit relay

standing + first-boundary gated; frames signed with this node's own
identity key (authorship = status.publicKey); reply held until the
validator answers or the SUBMIT_HOLD sweep expires it honestly.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: E2E — a resident posts to chat

**Files:**
- Create: `bin/node/tests/resident_submit_e2e.rs`
- Modify: `docs/validator-onboarding.md` — one paragraph: resident standing now includes writes (relayed, authorship = the resident's key); reads were already local.

**Interfaces:**
- Consumes: everything above; `NetworkShapeCluster` (`bin/node/tests/common/mod.rs`); `chat::interface` (dependency of the `ducktape-node` bin — reuse via `chat::…` path used by main.rs; if the test needs it in dev-dependencies, add `chat = { path = "../../crates/apps/chat" }` to `bin/node/Cargo.toml` `[dev-dependencies]`).

- [ ] **Step 1: Write the failing e2e**

```rust
//! resident submit relay, end to end on the network-shape cluster: a parked
//! joiner cannot write; once granted RESIDENT standing it posts to chat
//! through its OWN surface — the frame relays to the founder, finalizes, and
//! the recorded author is the RESIDENT's key (authorship rides the frame
//! signature, not the injecting validator). a member-gated module op from
//! the same resident finalizes Rejected — the relay grants no authority.

mod common;

use std::time::Duration;

use chat::interface::{
    Block, ChatMsg, ChatQuery, ChatReply, PostPolicy, decode_reply, encode_msg, encode_query,
    AuthorRef,
};
use common::{NetworkShapeCluster, serial};

#[test]
fn resident_posts_to_chat_with_its_own_authorship() {
    let _serial = serial();
    let mut cluster = NetworkShapeCluster::new();

    let chain_id = cluster.init_founder("resident-submit");
    assert!(!chain_id.is_empty());
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));

    // the founder opens the room BEFORE the friend even exists — policy Open,
    // so posting needs no chat membership, only authenticated authorship.
    cluster.submit(
        0,
        "chat",
        &encode_msg(&ChatMsg::CreateChannel {
            channel_id: "general".into(),
            name: "general".into(),
            post_policy: PostPolicy::Open,
        }),
    );

    let invite = cluster.invite_manual();
    let friend_key = cluster.join_friend(&invite);
    cluster.spawn(1);
    cluster.wait_marker(1, "parked:", Duration::from_secs(60));

    // WHILE PARKED (no standing): writes are refused with the parked wording.
    let refused = cluster.rpc(
        1,
        serde_json::json!({
            "cmd": "submit",
            "target": "chat",
            "payload_hex": common_hex(&encode_msg(&post("m-parked", "too early"))),
        }),
    );
    assert_eq!(refused["ok"], false, "parked node must refuse writes: {refused}");

    // grant RESIDENT standing (invite-accept = AddResident), wait for the
    // follow arm to pre-sync a boundary — the write gate needs both.
    let (ok, out) = cluster.run_membership_verb("invite-accept", &friend_key);
    assert!(ok, "invite-accept failed:\n{out}");
    cluster.wait_marker(1, "resident: standing granted", Duration::from_secs(120));
    cluster.wait_marker(1, "resident: pre-synced boundary", Duration::from_secs(120));

    // THE POINT: the resident posts through its OWN surface and the reply is
    // the relayed op's consensus fate (ok == Applied).
    let posted = cluster.rpc(
        1,
        serde_json::json!({
            "cmd": "submit",
            "target": "chat",
            "payload_hex": common_hex(&encode_msg(&post("m-resident", "hi from the cheap seats"))),
        }),
    );
    assert_eq!(posted["ok"], true, "resident submit should relay + apply: {posted}");

    // the founder's view of the message carries the RESIDENT's authorship.
    let raw = cluster
        .query(
            0,
            "chat",
            &encode_query(&ChatQuery::MessagesLatest {
                channel_id: "general".into(),
                limit: 10,
            }),
        )
        .expect("founder answers chat queries");
    let ChatReply::Messages(views) = decode_reply(&raw).expect("chat reply decodes") else {
        panic!("expected Messages");
    };
    let ours = views
        .iter()
        .find(|v| v.head.message_id == "m-resident")
        .expect("the relayed post finalized into the channel");
    let friend_bytes = unhex_pub(&friend_key);
    assert_eq!(
        ours.head.author,
        AuthorRef::User(friend_bytes),
        "authorship is the resident's key, not the injecting validator's"
    );

    // NO AUTHORITY ESCALATION: a member-gated governance op from the resident
    // finalizes Rejected, and the relay reply says so.
    let gov = cluster.rpc(
        1,
        serde_json::json!({
            "cmd": "submit",
            "target": "governance",
            // a syntactically-valid proposal from a NON-MEMBER origin: the
            // governance module rejects it deterministically at execute time.
            "payload_hex": common_hex(&governance_probe()),
        }),
    );
    assert_eq!(gov["ok"], false, "member-gated op must not apply: {gov}");

    cluster.kill(1);
    cluster.kill(0);
}

fn post(id: &str, text: &str) -> ChatMsg {
    ChatMsg::PostMessage {
        channel_id: "general".into(),
        message_id: id.into(),
        blocks: vec![Block::paragraph(text)],
        thread: None,
        as_agent: None,
    }
}

fn governance_probe() -> Vec<u8> {
    use governance::{GovAction, GovMsg, encode_msg};
    encode_msg(&GovMsg::Propose {
        proposal_id: "resident-escalation-probe:0".into(),
        action: GovAction::AddResident { key: vec![0xAA; 32] },
        voting_period: 1_000,
    })
}

fn common_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex_pub(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}
```

Adjust to the harness's actual exports: `common/mod.rs` may already export `hex`/`unhex` — use those instead of the local helpers if public. If `chat`/`governance` are not yet dev-dependencies of the bin, add to `bin/node/Cargo.toml` `[dev-dependencies]` (both are workspace path crates; mirror how existing e2es import `serde_json`).

- [ ] **Step 2: Run to verify it fails only where expected**

Run: `cargo test -p ducktape-node --test resident_submit_e2e -- --nocapture`
Expected before Tasks 3–4 land: the parked refusal passes, the resident submit FAILS (refused). After Tasks 3–4: full pass. (If executing tasks in order, this test is written last — expect a full pass; flip to red by reverting if independent verification of the red state is wanted. The unit tests in Tasks 1–2 carried the TDD red phase for the mechanics.)

- [ ] **Step 3: Run the sibling e2es that share the harness**

Run: `cargo test -p ducktape-node --test live_admission_e2e --test join_request_e2e -- --nocapture`
Expected: pass (serial-gated; run on an otherwise idle machine — parallel cluster e2es flake, dedicated runs are authoritative).

- [ ] **Step 4: Docs touch** — `docs/validator-onboarding.md`: in the resident-standing section, state: residents now WRITE through their own surface (the node signs with its identity key and relays to a validator; authorship is the resident's key; member-gated modules still reject non-member origins deterministically).

- [ ] **Step 5: Commit**

```bash
git add bin/node/tests/resident_submit_e2e.rs bin/node/Cargo.toml docs/validator-onboarding.md
git -c commit.gpgsign=false commit -m "test(node): resident submit relay e2e — chat authorship + no escalation

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: Full verification + ship

- [ ] **Step 1:** `cargo fmt --all && cargo clippy --workspace --all-targets 2>&1 | tail -20` — fix anything new.
- [ ] **Step 2:** `cargo test --workspace` (expect the pre-existing `cluster_upgrade_aborts` failure documented in memory — everything else green; relay/kernel/e2e suites all pass).
- [ ] **Step 3:** `make install` if the app build gates on it (tsconfig gate — no TS touched here, should be unaffected).
- [ ] **Step 4:** Push + PR to `dev`:

```bash
git push -u origin feat/observer-submit-relay
gh pr create --base dev --title "feat: resident submit relay — writes for resident-standing nodes" --body "..."
```

PR body covers: spec + plan paths, the authorship-rides-the-signature argument, the no-escalation e2e, flag-day note (new static channel 3).

---

## Self-Review Notes

- Spec coverage: wire (T2/T3), kernel custody (T1), validator gate+reply (T3), resident surfaces+seq+holds (T4), e2e incl. negative authority (T5), docs+mixed-version note (T5/T6, spec §Mixed versions needs no code). Spec's "+5s sweep" superseded — amended in T4 Step 8.
- Type consistency: `RelayHold` enum local to the serve window; `relay::RelayMsg/RelayOutcome` shared; `node::FrameId = [u8;32]` used as the wire `frame_id` directly.
- The borrow-shape CAUTIONs in T4 are real risks flagged for the implementer, not placeholders — the behavior on each path is fully specified.
