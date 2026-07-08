# Unified All-Paths Invite — Implementation Plan (on PR #260, simplified)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** One invitation bundles every entry path the inviter offers (direct + coordinated + member-fronts); the joiner races them and uses whichever works, failing honestly only if all fail. Closes the fully-NATed (incl. symmetric) inviter gap left by #260. No relay, no rendezvous cap, zero consensus change.

**Architecture:** Extend #260's seam. Add a `fronts` list to the invite (off-fingerprint), replace the joiner's exclusive direct/coordinated `match` with a union candidate race reusing #260's `BootstrapCoordinatedInvitePeer` + direct announcer, and populate fronts at mint from persisted `mesh-state.json`.

**Tech Stack:** Rust; `bin/node`, `reachability`. ed25519/X25519, serde_json intro wire, tokio.

## Global Constraints

- No coordinator relay (PR #173); keyless coordinator. **No rendezvous cap / private-coordinator work — CUT.**
- Zero consensus: `fronts` live OUTSIDE `genesis_namespace` (config.rs:263-286 hashes scheme + sorted validators only). Prove with a fingerprint-exclusion test.
- Build ON #260 — reuse `BootstrapCoordinatedInvitePeer`, `send_datagram_and_recv`, the direct announcer, the `(None,None)`/`(Some,Some)` branches. Do NOT overload `Reach::Fronted`.
- Crate layering: the datagram seam is already opaque in #260 (`SocketEvent::Datagram`); don't change it.
- node tests: `cargo test -p node <filter>`. reachability: `cargo test -p reachability`. Per-crate clippy `-p <crate> --tests --no-deps`; no `cargo fmt --all`.
- No mono-files: new module `bin/node/src/first_contact_join.rs`; don't grow `bin/node/src/main.rs` with large blocks.

---

## File Structure

- `bin/node/src/config.rs` — MODIFY: `Front` + `fronts: Vec<Front>` in `Invite` pack/unpack; fingerprint-exclusion test.
- `bin/node/src/first_contact_join.rs` — CREATE: union candidate builder + race + honest terminal.
- `bin/node/src/main.rs` — MODIFY (glue): call the ladder from the join branch; `cmd_invite` populates `fronts` from mesh-state.
- `crates/system/reachability/src/orchestrator.rs` — MODIFY (only if a small racing/snapshot helper is cleaner there).
- `bin/node/tests/coordinated_invite_cli.rs`, `crates/system/reachability/tests/orchestrator_e2e.rs` — MODIFY: extend.

---

## Task 1 — Invite carries fronts (`config.rs`)

**Files:** Modify `bin/node/src/config.rs`. Test: in-file `#[cfg(test)]`.

**Interfaces:**
- Produces: `pub struct Front { pub member_key:[u8;32], pub wireguard_public_key:[u8;32], pub mesh_port:u16, pub endpoint:Option<String> }`; `Invite` gains `pub fronts: Vec<Front>`; pack/unpack appended after the wireguard block.

- [ ] **Step 1 — failing tests:** (a) round-trip an invite with 2 fronts (one `endpoint Some`, one `None`); (b) a pre-feature blob (no fronts tag) still decodes to `fronts: vec![]`; (c) **fingerprint-exclusion**: two invites differing only in `fronts` yield the same `genesis_namespace`.
- [ ] **Step 2 — run, expect FAIL:** `cargo test -p node config`.
- [ ] **Step 3 — implement:** append a length-prefixed `fronts` block inside the issuer-signed envelope; never feed it to `genesis_namespace`. Fail-closed decode. Mirror `InviteWireGuard` pack style.
- [ ] **Step 4 — run, expect PASS.**
- [ ] **Step 5 — clippy `-p node --tests --no-deps` scope-clean; commit** `feat(node): invite carries member fronts (off-fingerprint)`.

---

## Task 2 — Joiner races the union (`bin/node/first_contact_join.rs`)

**Files:** Create `bin/node/src/first_contact_join.rs`; Modify `main.rs` (replace the `match (&wg.endpoint,&wg.intro)` at ~5920 with a call into the module). Test: unit for candidate building + selection.

**Interfaces:**
- Consumes: Task 1 `Front`; #260 `ReachabilityCommand::BootstrapCoordinatedInvitePeer`; the direct announcer; `lobby::{encode_intro, decode_intro_ack}`; `config::primary_coordinator_or_default`.
- Produces: `struct Candidate { key: ed25519::PublicKey, wg:[u8;32], mesh_port:u16, endpoint: Option<String> }` (one shape — `endpoint Some` ⇒ direct, `None` ⇒ coordinated-by-key); `fn build_candidates(invite) -> Vec<Candidate>` (= inviter ∪ fronts); `async fn drive_first_contact(candidates, resolver_cmds, intro, effect_kind, window) -> FirstContactOutcome { Installed{ key, via }, Terminal{ tried:usize, reason:String } }`.
- **Coordinator is ambient:** the resolver's coordinators come from `primary_coordinator_or_default` (config/default), NOT from the invite. Remove the invite→coordinator extraction at `main.rs:5870-5871` for the join path; bind the configured/default coordinator instead.

- [ ] **Step 1 — failing tests:** (a) `build_candidates` on an inviter-only invite (no fronts) yields exactly one candidate (the inviter); (b) on a unified invite (inviter + 2 fronts) yields 3; a front with `endpoint None` is coordinated, with `endpoint Some` is direct; (c) `drive_first_contact` returns `Installed` for the first candidate to ack and does not wait on the rest; (d) all fail → `Terminal{tried:N}` with a reason; (e) a coordinated candidate under TUN mode is dropped from the race (only one → `Terminal`, no hang).
- [ ] **Step 2 — run, expect FAIL:** `cargo test -p node first_contact`.
- [ ] **Step 3 — implement:** `build_candidates` (inviter ∪ fronts, one `Candidate` shape); bind the resolver coordinators from `primary_coordinator_or_default` (drop the invite→coordinator extraction); `drive_first_contact` races with bounded fan-out (`endpoint None` → `BootstrapCoordinatedInvitePeer{peer:key,wireguard_public_key:wg,intro}` via the ambient coordinator; `endpoint Some` → the announcer to `endpoint`'s intro port); first `IntroAck.installed` wins, cancel the rest; inject the winner's overlay ULA Direct hint here; honest `Terminal` on exhaustion with a mode-naming log. Move the #260 branch bodies into the module; `main.rs` just calls it.
- [ ] **Step 4 — run, expect PASS.**
- [ ] **Step 5 — clippy scope-clean; commit** `feat(node): joiner races the invite's union of paths + honest terminal`.

---

## Task 3 — Mint bundles fronts (`bin/node/main.rs cmd_invite`)

**Files:** Modify `bin/node/src/main.rs` (`cmd_invite` ~3611-3720). Test: extend the config/cli test — an invite minted with a seeded `mesh-state.json` carries fronts.

**Interfaces:**
- Consumes: `reachability::store::load` (persisted mesh state), Task 1 `Front`.
- Produces: `cmd_invite` reads mesh-state, filters to members with a concrete routable `wireguard_endpoint` (host-capable) OR punchable (registered, default wg_port+1), maps them to `Front`, and packs them. Keeps the inviter's own direct + coordinated paths both when available.

- [ ] **Step 1 — failing test:** `cmd_invite` with a seeded `mesh-state.json` (two reachable members) emits an invite whose decoded `fronts` has those two; with no mesh state, `fronts` is empty and a warning is printed (exit 0).
- [ ] **Step 2 — run, expect FAIL:** `cargo test -p node invite`.
- [ ] **Step 3 — implement:** load mesh-state, filter+map to `Front`, pack; warn-and-empty when absent. **Stop embedding a coordinator address** in the invite's reach hints (the inviter still registers with its own coordinator via its own config; the joiner uses its ambient coordinator). Document the widened-invite exposure in `docs/deploy/`.
- [ ] **Step 4 — run, expect PASS.**
- [ ] **Step 5 — clippy scope-clean; commit** `feat(node): cmd_invite bundles reachable-member fronts from mesh state`.

---

## Task 4 — Tests / e2e

**Files:** Modify `crates/system/reachability/tests/orchestrator_e2e.rs`, `bin/node/tests/coordinated_invite_cli.rs`; add an `ops/` leg. Docs: flip `docs/deploy/private-cutover-integration-gap.md`.

- [ ] **Step 1:** orchestrator `StaticResolver` test of `BootstrapCoordinatedInvitePeer` (resolve→install→ack) — none exists.
- [ ] **Step 2:** extend `coordinated_invite_cli.rs`: a fronts-carrying invite; a coordinated-only invite on a TUN node fails with the honest message.
- [ ] **Step 3:** `ops/` leg — NATed socket-mode inviter + public coordinator (T1 e2e, closes #262); symmetric inviter + one reachable front; all-symmetric → honest fail (non-zero, no silent success).
- [ ] **Step 4 — run** the new tests green.
- [ ] **Step 5 — commit** `test(e2e): unified-invite onboarding — T1 verify, fronts fallback, honest terminal`; flip the gap-doc status.

---

## Self-review (at authoring)

- Coverage: fronts in invite (T1), union race + honest terminal (T2), mint bundling (T3), e2e (T4), fingerprint-exclusion (T1). All spec sections map. Cap/private-coord explicitly cut.
- Placeholders: none — concrete test contracts + signatures per task.
- Type consistency: `Front`, `Candidate`, `build_candidates`, `drive_first_contact`, `FirstContactOutcome` used consistently.

## Execution order

Task 1 (invite fronts) → Task 2 (joiner race) → Task 3 (mint) → Task 4 (e2e). Sequential; all in `bin/node` except the one orchestrator test.
