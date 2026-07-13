# Statesync Fail-Closed Implementation Plan (PR6 / ADR §5.1)

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use `- [ ]`.

**Goal:** A serving validator refuses statesync/manifest to any requester whose REAL key is not in committed standing (validators ∪ residents) — so a valid targeted invite alone leaks ZERO chain state before admission (ADR R4, §5.1).

**Normative spec:** `docs/adr/2026-07-13-join-protocol.mdx` §5.1. This is the server-side half of R4; the client-side (joiner makes no sync attempt pre-admission) already shipped in #560.

**The decisive architecture fact (verified):** a parked joiner and an admitted-but-not-yet-rebooted resident present the SAME derived **lobby** transport key on `CHANNEL_STATE_SYNC` (`boot/mesh.rs` picks the lobby key while `joiner`, which stays true until the promotion reboot). A transport-key gate is therefore impossible. But the node still HOLDS its real signer while transporting under the lobby key (exactly as `GateMsg::Request` proves the real joiner key over the lobby channel). So enforcement keys off a **request-level proof of possession of the real key**, checked against committed standing.

**Why a static per-session proof is sound (bake into the review):** `CHANNEL_STATE_SYNC` rides commonware-p2p `authenticated::discovery` — the transport is authenticated AND encrypted, so a third party cannot capture a member's proof off the wire. The only adversary is someone who legitimately holds the lobby key (an invite holder) and wants to pull state pre-admission; they hold ONLY their own real key (the invite target), which is not in the valset until `Redeem` commits, so they cannot produce a proof for any standing key. A member handing its proof to an outsider is not a new vector (a member can already relay state). Therefore the proof may be signed ONCE over the genesis namespace and cached — no per-request signing cost, no per-request nonce.

**Blast radius (highest in the campaign):** statesync is how EVERY joiner, restart, and validator backfill obtains state. A mistake refuses legitimate sync and wedges the network. The restore path (a node with persisted standing) and validator backfill dial under their REAL keys, which ARE in the valset — the e2e MUST prove both still sync. Enforcement is a hard flag day (rejects old clients); all nodes update together.

## Global Constraints

- Branch `statesync-fail-closed`, worktree `.worktree/statesync-fail-closed` (forked post-#563). PR against `dev`. Commit after every task (a 0-commit worktree gets swept).
- Build via `ops/build-with.sh cargo ...`; `ulimit -s unlimited` before cargo fixes the rustc SIGSEGVs. Packages: `statesync` (crate name — verify with `head -3 crates/kernel/statesync/Cargo.toml`), `node-bin`.
- Gates per touched crate: `cargo clippy -p <crate> --tests --no-deps`. The statesync crate is kernel — run its unit tests. `-p simnode` is NOT needed (simnode doesn't run the p2p sync serve loop) but confirm it still compiles.
- NO backward compatibility: the RPC envelope changes in place; old clients/servers do not interop. Flag day.
- Do NOT weaken the restore/backfill path. Both e2e directions are load-bearing.

## Key files (verified anchors, may drift ±20 lines)

- Wire: `crates/kernel/statesync/src/lib.rs` — `encode_rpc`/`decode_rpc` (:918/:925), `SyncRequest` codec (:457/:516), `WireError`.
- Client: `crates/kernel/statesync/src/p2p.rs` — `P2pSyncClient::with_sources` (:128), `request` (:211).
- Server serve loop: `bin/node/src/validator/wiring.rs:224-280` (the NOTE explaining the transport-key impossibility is at :253-265 — update it), `bin/node/src/sync/serve.rs`.
- Standing reads: `bin/node/src/host_reads.rs:9-38` (`read_valset_members`, `read_valset_residents`).
- Real signer availability: the park/replica loop holds `signer` (its real key) even while transporting under the lobby key — the same `signer` `GateMsg::Request` signs with.
- PoP pattern to copy: `config::sign_join_proof`/`verify_join_proof` (`bin/node/src/config/invite.rs`), `lobby::gate_request`/`verify_join_request`.

---

### Task 1: Authenticated RPC envelope (statesync wire)

**Files:** `crates/kernel/statesync/src/lib.rs` (`encode_rpc`/`decode_rpc` + a new signing namespace const + tests).

**Interfaces produced:**
```rust
pub const SYNC_AUTH_NAMESPACE: &[u8] = b"ducktape-statesync-auth-v1"; // ed25519 sign namespace
// envelope layout in place: requester(32) ‖ proof(64) ‖ id(8 LE) ‖ body
pub fn encode_rpc_authed(requester: &[u8; 32], proof: &[u8; 64], id: u64, body: &[u8]) -> Vec<u8>;
// returns the requester key + proof + id + body slice; errors on truncation.
pub fn decode_rpc_authed(bytes: &[u8]) -> Result<(&[u8;32], &[u8;64], u64, &[u8]), WireError>;
```
Keep the old `encode_rpc`/`decode_rpc` name OR replace — since this is a flag day, REPLACE `encode_rpc`/`decode_rpc` with the authed form (no dual path). The signature over the binding is produced/verified by the caller (client/server), NOT inside the codec — the codec only frames bytes.

**Steps:** TDD. (1) roundtrip test: `decode_rpc_authed(encode_rpc_authed(...))` returns the same requester/proof/id/body; truncated buffers (< 32+64+8) error `Truncated`. (2) implement. (3) fix the two call sites of the old `encode_rpc`/`decode_rpc` to compile (client Task 2, server Task 3 wire them properly). (4) statesync crate unit tests green. (5) commit.

### Task 2: Client signs the proof once, attaches to every request

**Files:** `crates/kernel/statesync/src/p2p.rs` (`with_sources` gains the real signer + namespace; `request` attaches the cached proof), and its construction site `bin/node/src/replica/park.rs` (pass the real `signer` + `namespace`).

**Design:** at `with_sources`, sign `SYNC_AUTH_NAMESPACE` over the genesis `namespace` bytes ONCE with the real signer; store `(requester_pubkey_bytes, proof_bytes)`. `request` builds every envelope with `encode_rpc_authed(&requester, &proof, id, body)`. No per-request signing.

**Steps:** unit test in p2p.rs — a client built with signer S produces envelopes whose decoded `requester == S.public_key()` and whose `proof` verifies under `SYNC_AUTH_NAMESPACE` over the namespace. Thread the signer at the park.rs construction site (the real `signer` is in scope there — the same one used for `GateMsg`). Build node-bin. Commit.

### Task 3: Server verifies PoP + standing, fail-closed

**Files:** `bin/node/src/validator/wiring.rs:224-280` (serve loop), maybe `bin/node/src/sync/serve.rs`.

**Design:**
- Decode with `decode_rpc_authed`. Verify the proof: `requester.verify(SYNC_AUTH_NAMESPACE, namespace, proof)`. Fail → drop the request (no reply, or a typed refusal — dropping is simplest and matches deny-by-default; log at debug for observability).
- Standing check: `requester ∈ (members ∪ residents)`. Use a snapshot captured in the serve scope and REFRESHED at each epoch cutover (the wiring already reads `initial_member_keys`/`initial_resident_keys` at :205-207 and `read_valset_residents` at :403 — capture a shared `Arc<...>`/channel-updated set, or re-read per request via the existing `SyncStateRequest` seam if simpler and cheap enough). Not in the set → refuse (drop). In the set → serve as today.
- Update the NOTE at :253-265: transport-key gate impossible (unchanged reason), now enforced via real-key proof + standing.

**Steps:** This is the load-bearing task. Prefer the simplest correct standing source: if `read_valset_residents`/`read_valset_members` are cheaply callable in the serve loop (they query local host state — confirm no deadlock with the serve task's own borrow of host), call them per request (rare enough — sync requests are not that hot, and the check is a local map lookup after one query). If per-request query is too heavy or borrow-conflicts, capture the snapshot at scope entry + refresh on the epoch-cutover signal the wiring already observes. Document which you chose and why. Build; commit.

### Task 4: e2e — the security property AND the two must-not-break paths

**Files:** extend `bin/node/tests/live_admission_e2e.rs` or a new `bin/node/tests/statesync_fail_closed_e2e.rs` using `NetworkShapeCluster`.

Three assertions, all load-bearing:
1. **Leaks zero chain state**: a node holding a valid targeted invite blob but NOT yet admitted (transports under the lobby key, real key not in valset) issues a manifest/chunk request and is REFUSED — it obtains no boundary. (Drive via the real join path but assert no sync completes before Admitted; or a focused harness that sends a sync request under a non-standing key.) This is the property PR6 exists for.
2. **Restore path still syncs**: a validator restarted with persisted standing (real key in valset) syncs normally — no regression.
3. **Admitted resident syncs**: after the gate grants standing (its real key enters residents), the joiner's boundary sync SUCCEEDS under the still-lobby transport key — proving the proof+standing path admits a legitimately-admitted resident despite the shared transport key.

The happy-path join e2e (`live_admission_e2e` staged_admission) already exercises (3) end to end IF the joiner's sync now carries the proof — so first confirm that test still passes (it will fail if the client doesn't sign or the server wrongly refuses an admitted resident). (2) is covered by the restart e2es (`restart_e2e`, `replica_restart_e2e`) — run them. Add (1) as the new focused test.

**Steps:** run the existing join + restart e2es FIRST (they prove 2+3 for free and catch a wrongly-refusing server immediately). Then add test (1). Commit.

### Task 5: Gates + PR

`ulimit -s unlimited` then: clippy (statesync, node-bin), statesync unit tests, the join/restart e2es + the new fail-closed e2e, files wasm gate, `-p simnode` compile check. Push, open PR. Body: the security property (targeted invite leaks zero chain state), the real-key-proof-not-transport-gate design + why static-per-session is sound (encrypted authenticated transport), the restore/backfill no-regression evidence, and the flag day. Cite ADR §5.1.

## Risks
- **Server borrow/deadlock**: reading valset from inside the serve task while the consensus loop holds host state — verify the existing `SyncStateRequest` seam or `read_valset_*` don't deadlock (the join gate reads the same from `on_lobby`, a different task, so the pattern is proven safe; mirror it). If in doubt, snapshot-at-scope + epoch-refresh avoids any per-request host touch.
- **Refusing a legitimate resident** = the worst failure (a just-admitted joiner can never sync → never promotes). Test (3) is the guard; if it fails, the standing snapshot is stale (not refreshed after the Redeem block) — refresh cadence must include the block that grants residency, not just epoch cutovers. Prefer per-request `read_valset_residents` unless proven too costly, precisely to avoid a stale snapshot starving a fresh resident.
