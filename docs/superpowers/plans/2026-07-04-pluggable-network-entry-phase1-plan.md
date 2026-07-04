# Pluggable Network Entry — Phase 1 (Sentry) — Implementation Plan

Implements Phase 1 of
`docs/superpowers/specs/2026-07-04-pluggable-network-entry-design.md`.

## Verdict from investigation

The sentry pattern **already works on the current code with configuration
alone.** A validator sets `advertised = <sentry_addr>` while `listen` stays
private; joiners carry the bootstrap hint `validator_key@sentry_addr`; the mesh
authenticates purely by ed25519 key with **no source-IP pinning**, so a
transparent TCP splice in front of a validator is invisible to admission,
handshake, and state-sync.

Evidence (direct source reads of commonware `authenticated::discovery` 2026.5.0):

- The encrypted handshake takes a `PublicKey`, never a `SocketAddr`
  (`commonware-stream` `encrypted.rs:187,246`).
- The dialer resolves the advertised ingress, dials it, and upgrades expecting
  the configured key — never comparing the socket's remote addr to the advertised
  addr (`dialer.rs:107-134`).
- The listener uses the observed source IP **only** for anti-DoS (private-IP
  gate + rate limits, `listener.rs:177-205`); acceptance is a predicate over the
  peer's public key (`listener.rs:98-104`).
- A peer's advertised address is a self-signed `Info{ingress,...}`; verification
  checks the signature only, never ingress-vs-observed-addr (`types.rs:361-390`).
  The node signs `cfg.dialable` (= advertised), never its `listen`
  (`tracker/actor.rs:75-78`).
- Commonware's own `test_peer_restart_with_new_address_must_dial`
  (`discovery/mod.rs:1900-1963`) sets `dialable != listen` (and even an
  unreachable advertised IP) and the mesh still forms — the exact sentry
  topology.

On the ducktape side, `listen`/`advertised` are already independent config
(`config.rs:723-729` network, `802-808` dev), no
`advertised != listen` guardrail exists, and `choose_sync_source`
(`config.rs:635-646`) selects the sync source by **key** only, so a sentry — which
has no key in the descriptor — can never be picked as a state-sync source.

**Therefore Phase 1 ships no production/consensus change.** The deliverable
converts an implicit capability into a regression-guarded, documented one.

## Scope decision: typed reach hint DEFERRED

A typed `Direct`/`Fronted` reach hint is **not** built in Phase 1.
`Direct` and `Fronted` are wire-identical ("dial this address, expect this
key"), so the tag carries zero behavioral difference now. The v2 invite is a
positional binary payload with a version byte (`config.rs:375,439-443`), so a
reach field cannot be added additively — it would force a flag-day v3 bump for no
gain. Defer the enum + v3 to Phase 2, when `Coordinated` actually needs to
distinguish reach on the wire.

## Delta

### 1. `bin/node/tests/common/mod.rs` — harness plumbing (test-only)

- Add `advertised: Vec<Option<String>>`, initialized in `Cluster::new` (~`266`)
  to all-`None`. In `config_path` (~`286-319`), when `advertised[idx]` is `Some`,
  emit `advertised = "<addr>"` immediately after the `listen` line. `None` emits
  nothing → identical to today.
- Add `bootstrap_addr_override: Option<String>` (default `None`). At both sites
  that build `bootstrapper_addr` — `config_path` (~`296-300`) and `spawn_joiner`
  (~`370-373`) — use
  `self.bootstrap_addr_override.clone().unwrap_or_else(|| format!("127.0.0.1:{}", self.p2p_ports[0]))`.
  `None` → identical to today.

Both fields default to current behavior, so every existing test is untouched.

### 2. `bin/node/tests/sentry_e2e.rs` — new regression test

- `mod common;` reusing the `Cluster` harness.
- Free fn `spawn_sentry(target: SocketAddr) -> (SocketAddr, Arc<AtomicU64>)`: a
  std `TcpListener` accept loop; per connection, `TcpStream::connect(target)` +
  two `std::io::copy` relay threads (one per direction) incrementing an
  `AtomicU64` byte counter. Pure `std::net`/`std::thread` — no new dependency
  (matches the harness idiom).
- One test modeled on `solo_founder_invites_a_friend` (`invite_e2e.rs:57`):
  network of ONE (`Cluster::new(&[0], &[0])`), node 0 = founder + sole validator,
  fronted by the sentry, plus one out-of-mesh joiner. This is the tightest proof —
  2-of-2 consensus finalizes only if the joiner reaches node 0, and node 0 is
  reachable only via the sentry.
- Flow: node 0's real listen is `127.0.0.1:p2p_ports[0]` (pre-allocated in
  `new()`); `spawn_sentry(node0_listen)`; set `cluster.advertised[0] =
  Some(sentry_addr)` and `cluster.bootstrap_addr_override = Some(sentry_addr)`
  **before** `spawn(0)`; `spawn(0)`; `spawn_joiner(1)`; run `invite-accept` on
  node 0; assert joiner markers `admitted at epoch 1`, `synced app_hash=`,
  `promoted: validator at epoch 1`; assert `counter.load() > 0` (real bytes
  transited the sentry); cross-read an op founder↔friend and assert identical
  status app-hashes (live consensus over the pipe, no fork).
- Sequencing: the sentry must be listening and its addr known before `spawn(0)`
  (config is written at spawn time); `alloc_ports` pre-binds node 0's real port
  in `new()`, so it is known early. Node 0's real listen stays bound; only
  `advertised` + others' bootstrap point at the forwarder, so no peer learns the
  private port.

### 3. `docs/sentry-deployment.md` — deployment recipes

- **(A) Forward sentry (Cosmos-style):** sentry listens public, TCP-splices to
  the validator's private `listen` (`nginx stream` / HAProxy TCP / small Rust
  forwarder). Validator sets `advertised = <sentry_public_addr>`, `listen =
  <private_addr>`; invites carry `validator_key@sentry_public_addr`.
- **(B) Reverse tunnel:** validator dials OUT to an edge (frp / rathole /
  `ssh -R` / cloudflared-style); the edge is the public face, no inbound port on
  the validator.
- Caveats: forward-sentry-on-a-private-network relies on the `local` preset's
  `allow_private_ips: true` (`main.rs:1595`); a switch to the `recommended`
  preset (`allow_private_ips: false`) would reject a forwarded connection from a
  private source IP — use a public-IP sentry or reverse tunnel then. A DNS-named
  edge is pinned to one IP at boot (`resolve_one`), fine for a static A-record.
  State-sync always terminates at a validator — the sentry is pure path.
- Cross-link the design of record.

### 4. `bin/node/src/main.rs` — one-line comment only (no behavior change)

At the `discovery::Config::local` call (~`1595`), add a comment noting that
forward-sentry deployments on a private network depend on this `local` preset's
`allow_private_ips: true`; switching presets would require public-IP or
reverse-tunnel sentries. This is the only production-file touch and it is a
comment.

## Verification

- `cargo build` for the node package (resolve the package producing
  `ducktape-node`).
- `cargo test` the new `sentry_e2e` target only (real-process e2e; reuse the
  sibling tests' generous convergence/finalize timeouts).
- `cargo test` a couple of existing `invite_e2e` cases to confirm the harness
  changes did not regress them.
- `git diff --check`.

## Risks (from investigation)

- `allow_private_ips` coupling for forward sentries → documented; one-line
  comment at `main.rs:1595`.
- DNS-named edge pinned at boot → noted in docs, not built for.
- 2-validator quorum teardown → keep the founder alive until cross-reads
  complete; use sibling timeouts.
- Real-process/real-TCP flakiness → reuse `alloc_ports`; sibling timeout
  constants.

## Out of scope (later phases)

Typed reach hint + v3 invite (Phase 2), coordinator/STUN (Phase 2), WireGuard
cutover/hole-punch (Phase 3).
