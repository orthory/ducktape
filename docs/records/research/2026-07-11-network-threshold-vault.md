# Network-Wide Threshold Key Custody ("Vault" / MPC Keybase) — Research

Date: 2026-07-11. Method: full repo map (consensus/governance, identity/custody, prior-art grep) + external survey of threshold-crypto systems (FROST/ROAST, Ferveo/Penumbra, Lit, drand, CHURP/DPSS, threshold-ECDSA, Fireblocks TAP). This is a research record, not a decision. If it hardens into one, promote to `docs/adr/`.

## Executive summary

1. **Greenfield, but not blank.** No threshold / DKG / MPC / secret-sharing code exists anywhere in the tree (hard grep: zero hits for `shamir|frost|dkg|vss|feldman|pedersen|mpc|threshold-share`). The only multi-party crypto is **BLS12-381 MinPk *multisig aggregation*** for V2 consensus certs — and the consensus code deliberately keeps it non-threshold.

2. **The one hard constraint drives the whole design.** Consensus explicitly *rejected* threshold-BLS because "those need DKG/resharing, which fights the epoch teardown-respawn contract" (`crates/kernel/consensus/src/lib.rs:100-101`; echoed in `docs/superpowers/specs/2026-07-04-no-downtime-node-upgrade-design.md:440`). Every membership change tears down and respawns the consensus engine with a fresh valset (`crates/kernel/consensus/src/valset_orchestrator.rs`). A distributed key held *by the validator set* would have to be reshared on every one of those respawns. **That rejection is correct, and it dictates the answer below.**

3. **Resolution: decouple the key-holding committee from the validator set.** Consensus + governance become the **control plane** (order ceremonies, record commitments, enforce policy). The threshold shares live in a **custody committee** whose membership is *governed by* consensus but is **not** the consensus engine — so shares persist across epoch respawns and reshare only on deliberate governance events, not on every epoch cutover. This is the single load-bearing architectural decision.

4. **Two capabilities are conflated under one name.** "MPC keybase / vault" means either (A) **threshold decryption / secret custody** — the network holds decryption shares; a threshold reads — or (B) **threshold signing** — the network holds a signing key and signs on behalf of the network/an account. (A) is a light upgrade of the existing `vaults` module; (B) is a heavier interactive ceremony (FROST+ROAST for native ed25519, threshold-ECDSA for external chains). **Recommend (A) first.**

5. **Everything except the threshold-crypto core already exists and is reusable.** Governance proposals/ballots, valset two-tier membership, invite PoP, genesis-issued `CoordCap` delegation, the `vaults` ciphertext-on-consensus precedent, the sealed-lanes ADR's planned node `/v1/crypto/{pubkey,seal,open}` routes, and the module SDK are all directly reusable as the control plane. The genuinely new code is: DKG, threshold sign/decrypt, and resharing.

6. **The crypto is buildable from in-tree deps.** BLS12-381 (blst + ark-bls12-381, already linked transitively) for threshold decryption; `frost-ed25519` (RFC 9591) for native Schnorr signing that verifies under the *existing* ed25519 verifiers; `cggmp21` (audited) only if external-chain ECDSA is needed. Resharing via drand-style Feldman/Desmedt-Jajodia or CHURP/DPSS, governance-triggered.

7. **Bonus unlock: network/social recovery of the user key.** User-key custody is mnemonic-only today and social recovery is explicitly out of scope (`docs/superpowers/specs/2026-07-07-user-node-identity-split-design.md:242`). A network custody committee is exactly the primitive that unlocks threshold recovery of a lost user key — arguably the highest-value product feature the vault enables.

---

## The crux: epoch teardown-respawn vs. DKG resharing

This is the whole research in one section. Read it before anything else.

Ducktape's consensus does **membership rotation by teardown-respawn**: a finalized valset change arms a `ScheduledCutover` (`valset_orchestrator.rs:56`); at the cutover view every node discards and rebuilds the simplex `Engine` with the new participant set via a `RespawnPlan` (`valset_orchestrator.rs:129`). The consensus scheme *and* validator set are fixed at `Engine` construction — you cannot mutate them in place.

Threshold cryptography wants the opposite. A `(t,n)` distributed key is generated once by DKG and must **survive** as members come and go, which requires **proactive / dynamic-committee resharing** (drand does this via Desmedt-Jajodia reshare; CHURP is the state-of-the-art dynamic-committee proactive scheme). If you make the **validator set == the key-holding committee** (the Ferveo / drand / Lit model), then *every* epoch cutover is also a reshare event — and worse, a naive respawn that regenerates node state would destroy the shares. That collision is precisely why the consensus code refuses threshold-BLS.

**Two ways out, and the recommendation:**

- **Fused (validator = key holder).** Ferveo/Penumbra run DKG per-epoch over the validator set, weighted by stake; drand's nodes *are* the threshold-BLS holders and reshare on membership change; Lit's network nodes hold PKP shares in TEEs. Clean conceptually, and "MPC over consensus" in the literal sense. **But it re-opens the exact conflict the codebase already rejected** — you'd owe a resharing protocol wired into the respawn path, and every routine validator churn becomes a cryptographic ceremony.

- **Decoupled (key committee ≠ validator set) — RECOMMENDED.** The custody committee is its own membership set, *seated and rotated by governance* but not by the consensus engine. Its shares are **module state** (part of the app-hash / snapshot), not consensus-engine state, so a respawn doesn't touch them. Resharing runs only when governance explicitly changes committee membership — far rarer than epoch cutover, and completely off the respawn hot path. The consensus log still orders every ceremony message, so you keep total-order coordination without coupling key lifetime to engine lifetime.

The decoupled model is the only one that respects the teardown-respawn contract instead of fighting it.

---

## Reusable substrate (what you don't have to build)

| Vault need | Already in tree | Ref |
|---|---|---|
| Order the DKG / sign / decrypt / reshare ceremony | Consensus `Orderer` — total-order broadcast of opaque frames; the log is a **non-equivocating coordinator** | `crates/kernel/node/src/lib.rs:706`, `consensus/src/lib.rs:1081` |
| Decide committee membership + access policy | `governance` Propose/Vote/Execute (strict-majority tally), sole author of membership ops | `crates/system/governance/src/{lib.rs:74,interface.rs:50}` |
| Two-tier membership to seat a committee | `valset` validators (quorum seat) vs residents (mesh/statesync, no seat) | `crates/system/valset/src/interface.rs:15` |
| Admit a member with a single-use proof | invite `InviteToken` + joiner PoP, re-verified on `Redeem` | `crates/system/governance/src/invite.rs` |
| Delegate a capability off a genesis root, TTL-bounded | keyless-coordinator `CoordCap{issuer∈genesis_set, not_after, sig}` | `crates/system/nat-traversal/src/auth.rs` |
| Ciphertext-on-consensus + reader ACL (the vault precedent) | `vaults` module — opaque ciphertext, client-side X25519 envelopes, ACL = write-integrity + reader bookkeeping | `crates/apps/vaults/`, registered `bin/node/src/host_state.rs:135` |
| Node-side crypto that keeps keys out of the webview | sealed-lanes ADR plans `/v1/crypto/{pubkey,seal,open}` and notes they'd "also finally serve vaults" | `docs/adr/2026-07-06-private-team-messaging.mdx` |
| Rich single-key custody to model share custody on | mnemonic=seed, argon2id → XChaCha20-Poly1305, zeroized session cache | `bin/node/src/userkey.rs` |
| Snapshot/install so a new committee member syncs shares | statesync `serve_sync` (committed-state-only, root-verified) | `crates/kernel/sdk/src/lib.rs:356` |
| A place to register the new module | canonical genesis registry | `bin/node/src/host_state.rs:97-188` |

**Naming collision to resolve:** `crates/apps/vaults` already exists and is explicitly *not* the MPC vault (console-redesign-spec: "Explicitly NOT doing … multisig treasury vault"). Recommend the threshold system be a **new module** (`keybase` or `custody`) that sits as a heavier tier *alongside* client-sealed `vaults` — don't overload the existing name.

---

## Design space (prior art)

**Threshold decryption over BFT — Ferveo / Penumbra.** Validators run DKG once per epoch; encrypt-to-network, decrypt-by-threshold after ordering (built for encrypted mempools / front-running protection). Uses BLS12-381 pairings (Ferveo) or ElGamal over decaf377 (Penumbra). Directly the "encrypt to the network, threshold decrypts" pattern — but fused-to-validators (see crux).

**Decentralized key-management network — Lit Protocol.** Nodes hold PKP (programmable-key-pair) shares from DKG; threshold BLS signing/decryption gated by "Access Control Conditions"; runs in SEV-SNP TEEs; 2/3 threshold. Closest *product* analog to "network keybase governed by policy." Note the TEE dependency — Ducktape has no TEE story, so its trust model must be pure-crypto + BFT, not enclave-assisted.

**Native Schnorr threshold — FROST / ROAST.** `frost-ed25519` (RFC 9591, ZF) produces *standard* ed25519 signatures from `(t,n)` shares — they verify under Ducktape's existing ed25519 verifiers with zero verifier changes. Two-round, semi-interactive, with DKG built in. **ROAST** wraps FROST into a robust *asynchronous* protocol (anti-DoS, no timeouts, tolerates offline/malicious signers) given a coordinator — and **the consensus log already is that coordinator**, so much of ROAST's machinery is free here.

**Threshold ECDSA (external chains) — CGGMP21 / DKLs23.** For signing Bitcoin/Ethereum txs (secp256k1). `cggmp21` (LFDT-Lockness, audited; DFNS) supports 1-round online signing + key-refresh + identifiable abort; DKLs23 (Silent Shard) is the other modern option; ZenGo `multi-party-ecdsa` is the older GG20 line. All much heavier (Paillier, range proofs). Build only if custodying external-chain assets is a product goal.

**Dynamic-committee resharing — CHURP / DPSS / drand.** CHURP is churn-robust proactive secret sharing for changing committees (O(n) on-chain optimistic). drand reshares via Feldman/Joint-Feldman VSS + Desmedt-Jajodia. This is the protocol that replaces "resharing on every epoch" with "resharing on governance membership change."

**Policy / governance layer — Fireblocks TAP.** The reference for "network decides who can trigger a signature": a transaction-authorization policy (allow/block/require-approvers by source/dest/asset/amount), a quorum of endpoints each validating against policy before contributing a share, signed audit log per invocation. Ducktape's analog is a **consensus-enforced policy module** — governance commits the policy, `execute` refuses any sign/decrypt request that fails it. No enclave needed because consensus is the enforcement boundary.

**Recovery — social-recovery wallets / Shamir.** Guardians (here: account member keys or a resident committee) hold shares; a threshold authorizes installing a replacement key. Maps cleanly onto the identity module's account/member registry.

---

## Recommended architecture: control plane / key plane split

```
                    CONTROL PLANE  (consensus + governance — reuse)
  ┌──────────────────────────────────────────────────────────────────┐
  │  keybase module (new Module)                                       │
  │   • governance seats/rotates the CUSTODY COMMITTEE (its own set)   │
  │   • policy state: who/what may trigger sign|decrypt (TAP analog)   │
  │   • every ceremony msg (DKG round, sign/decrypt request, reshare)  │
  │     is an ORDERED OP → the log is the non-equivocating coordinator │
  │     (ROAST-robustness for free; retries are just more ops)         │
  │   • commitments/transcripts + verification live in app-hash        │
  └───────────────────────────────┬──────────────────────────────────┘
                                   │  ordered ceremony frames
                    KEY PLANE  (new — the only genuinely novel code)
  ┌───────────────────────────────┴──────────────────────────────────┐
  │  custody committee nodes hold (t,n) SHARES as module state        │
  │   • DKG (Pedersen/Feldman) → collective public key                │
  │   • threshold DECRYPT (BLS12-381 TPKE)  ← Phase 1                  │
  │   • threshold SIGN   (FROST-ed25519)    ← Phase 3                  │
  │   • RESHARE on governance membership change ONLY, not per-epoch    │
  │   • shares survive engine respawn (they are vault state, not       │
  │     consensus-engine state) — this is what dodges the crux         │
  └───────────────────────────────────────────────────────────────────┘
```

Why "ceremony-as-ordered-ops" is the keystone: FROST/ROAST and DKG both need a coordinator that can't equivocate about who said what and in what order. Ducktape's consensus log *is* that — every share/round is a committed frame, identical on every node, replayable. You get anti-equivocation, robustness, and an audit trail without a separate coordinator service. The custody nodes just react to the ordered stream.

---

## Crypto menu (grounded in in-tree deps)

| Capability | Scheme | Library | In-tree today | Weight |
|---|---|---|---|---|
| Threshold **decryption** / secret custody | BLS12-381 TPKE (Ferveo-style) or threshold ElGamal | blst / ark-bls12-381 (via commonware) or curve25519-dalek | ✅ linked | medium |
| Threshold **signing**, native identity | FROST (Schnorr) → **standard ed25519 sigs** | `frost-ed25519` (RFC 9591) | ➕ new dep, matches ed25519 | medium |
| Robust async signing over the mesh | ROAST wrapper on FROST | thin wrapper; log = coordinator | ➕ mostly free | low-medium |
| Threshold **signing**, external chains | threshold ECDSA (CGGMP21 / DKLs23) | `cggmp21` (audited) / Silent Shard | ➕ heavy new dep | high |
| DKG | Pedersen / Feldman VSS | in FROST (`frost-core`) or drand-style | ➕ | medium |
| Resharing on committee change | Feldman + Desmedt-Jajodia, or CHURP/DPSS | custom over the DKG primitives | ➕ | high |

Lazy read: **BLS12-381 for decryption reuses a curve consensus already links; FROST-ed25519 for signing verifies under existing verifiers.** Those two choices minimize new surface. External-chain ECDSA is the one genuinely heavy dependency — gate it behind an actual asset-custody requirement.

---

## Phasing (build order — and what NOT to build)

- **Phase 0 — control plane, no threshold crypto yet.** New `keybase` module: committee membership via governance, policy state (allow/deny sign|decrypt by principal), ceremony-frame plumbing over the log. Prove it with a **trusted-dealer single key** (dealer splits, distributes shares as ordered ops). This validates the whole orchestration with zero exotic crypto. *Ship this alone as the skeleton.*
- **Phase 1 — DKG + threshold decryption.** Replace the trusted dealer with real DKG; implement threshold decrypt (BLS TPKE). This is the vault MVP: **network-held secret custody with governance-gated reads**, a strict upgrade over client-sealed `vaults`.
- **Phase 2 — resharing on governance membership change.** The dynamic-committee piece. Decoupled from epoch cutover by construction.
- **Phase 3 — FROST signing.** Native ed25519 threshold signatures + ROAST robustness. Enables "the network/account signs on behalf of an absent member."
- **Phase 4 (only on demand):** external-chain threshold ECDSA; **network/social recovery of the user key** (high product value, folds into the identity module).

**Do NOT build up front:** external-chain ECDSA (heaviest dep, no stated use case), a TEE trust story (Lit's model; consensus is our enforcement boundary instead), forward secrecy / ratcheting (sealed-lanes ADR already scoped this out), a bespoke resharing scheme before Phase 1 even has a key to reshare.

---

## Load-bearing decisions (the forks to settle before building)

1. **Committee = validators, or decoupled set?** Recommendation: **decoupled** (respects teardown-respawn). Fused is simpler on paper but re-opens the rejected DKG-vs-epoch conflict.
2. **Primary capability: decryption-first or signing-first?** Recommendation: **decryption-first** (light, ~upgrades `vaults`, immediate custody value). Signing is heavier; external-chain signing heaviest.
3. **New `keybase`/`custody` module, or overload `vaults`?** Recommendation: **new module**, keep `vaults` as the client-sealed tier — avoid the naming/semantics collision.
4. **Share custody at rest.** Committee shares are hot secrets read at every ceremony; follow the **node-key model** (plaintext 0600, disposable, recovered by reshare) rather than the user-key model (password-gated) — a password prompt would stall liveness, same reasoning as `identity-onboarding-design.md`.

---

## References

Codebase (authoritative):
- Consensus rejection of threshold-BLS: `crates/kernel/consensus/src/lib.rs:100-101`; `docs/superpowers/specs/2026-07-04-no-downtime-node-upgrade-design.md:440`
- Epoch teardown-respawn: `crates/kernel/consensus/src/valset_orchestrator.rs`
- Governance: `crates/system/governance/src/{lib.rs,interface.rs,invite.rs}`; valset tiers `crates/system/valset/src/interface.rs:15`
- CoordCap delegation: `crates/system/nat-traversal/src/auth.rs`
- Existing `vaults`: `crates/apps/vaults/`; sealed-lanes ADR: `docs/adr/2026-07-06-private-team-messaging.mdx`
- Custody / keys: `bin/node/src/userkey.rs`; identity registry: `crates/system/identity/src/lib.rs`; module SDK: `crates/kernel/sdk/src/lib.rs:320`

External:
- FROST: [ZcashFoundation/frost](https://github.com/ZcashFoundation/frost), [RFC 9591](https://datatracker.ietf.org/doc/rfc9591/), [frost-ed25519](https://crates.io/crates/frost-ed25519)
- ROAST: [eprint 2022/550](https://eprint.iacr.org/2022/550), [Blockstream writeup](https://blog.blockstream.com/roast-robust-asynchronous-schnorr-threshold-signatures/)
- Ferveo: [anoma/ferveo](https://github.com/anoma/ferveo), [eprint 2022/898](https://eprint.iacr.org/2022/898.pdf); Penumbra DKG: [protocol.penumbra.zone](https://protocol.penumbra.zone/main/crypto/flow-encryption/dkg.html)
- Lit Protocol: [developer.litprotocol.com](https://developer.litprotocol.com/user-wallets/pkps/overview)
- CHURP (dynamic-committee proactive secret sharing): [par.nsf.gov/servlets/purl/10153196](https://par.nsf.gov/servlets/purl/10153196)
- drand (threshold BLS + DKG + resharing): [drand/drand](https://github.com/drand/drand), [cryptography docs](https://docs.drand.love/docs/cryptography/)
- Threshold ECDSA: [LFDT-Lockness/cggmp21](https://github.com/LFDT-Lockness/cggmp21), [DFNS CGGMP21](https://dfns.co/article/cggmp21-in-rust-at-last)
- Policy engine analog: [Fireblocks Governance & Policy Engine](https://www.fireblocks.com/platforms/governance-and-policies)
