# Multisig Vault + Browser Wallet — Campaign Plan

Design: `docs/superpowers/specs/2026-07-12-multisig-vault-browser-wallet-design.md`

Six PRs. **PR-0 and PR-1 are independent and run in parallel.** PR-0 is a
hard prerequisite for anything that renders public web content, and closes a
standing BLOCKER from the gateway-v2 audit — it is worth landing even if the
rest of the campaign is dropped.

## PR-0 — Control-plane listener hardening

**Why first:** `bin/noded/src/lib.rs` serves `/v1/submit`, `/v1/query`,
`/v1/files/*`, `/v1/fs/*`, `/forge/*` from one router with no origin guard and
permissive CORS. A public page in a CEF webview can `fetch` it and forge
consensus ops as the node. Nothing that opens the browser to the internet may
land before this.

- Bearer token at `~/.ducktape/<ws>/api.token`, required on every `/v1`
  request. One axum middleware.
- Update local callers to read it: CLI, agents, the console, `git push` to
  forge (`app/src-tauri/src/forge_git.rs`), `bin/noded` tests.
- Strict CORS replacing the permissive header.
- Wallet/gateway bridge split onto its own listener (routes added in PR-4;
  this PR creates the listener and moves `/v1/gateway/*` to it).

**Gate:** a test asserting an unauthenticated `/v1/submit` is refused; CLI,
forge push, and the console still work; fleet comes up clean.

**Risk:** touches every `/v1` caller in the tree. Land it alone.

## PR-1 — `crates/apps/multisig` (consensus, pure)

Named `multisig` to avoid colliding with `crates/apps/vaults` (client-sealed
secrets, a different thing).

- State + ops per design §4: `RegisterVault`, `BindOwnerEoa`, `ProposeTx`,
  `Approve`, `RecordExecution`.
- `safe.rs`: EIP-712 SafeTx hash construction, `execTransaction` calldata
  encoding. All Safe-specific code lives here — this is the MPC seam.
- Monotonic per-vault nonce allocation.
- `Approve` verifies by ecrecover against the bound owner EOA (`k256`).
- HKDF-derived secp256k1 key in `bin/node/src/userkey.rs` (domain-separated
  from the ed25519 seed).
- Register in `bin/node/src/host_state.rs`.

**Gate:** SafeTx-hash vectors match Safe's published test vectors — a mistake
here silently produces unspendable approvals. Nonce allocation under
concurrent proposals. `Approve` rejects: non-owner, owner-over-wrong-hash,
signature replayed onto another proposal.

**No I/O.** Every op is a pure function of committed state.

## PR-2 — Chain I/O (saga effect + oracle worker)

Reuses the `crates/system/dispatch-oracle` pattern: host-side worker runs the
impure call, submits the answer back as an ordered op.

- Read: Safe nonce (seeds `next_nonce` at registration), balances, owner set
  (mirror-drift detection).
- Broadcast: on threshold, submit `execTransaction`; report the receipt as
  `RecordExecution`.
- RPC endpoint per `chain_id` in settings. Exactly one node broadcasts
  (foreign-lease skipping, as dispatch-oracle already does).

**Gate:** anvil fork — broadcast lands, receipt recorded, drift detected.

## PR-3 — Browser reaches the internet

**Depends on PR-0.**

- Lift the `.duck`-only host gate in `app/src/domain/duck-browser.ts` and the
  single-origin `on_navigation` pin in `app/src-tauri/src/gateway_window.rs`.
- Address bar, navigation, and the minimum browser UX to be usable.
- Per-origin permission model (the wallet's connect permission rides this in
  PR-4).
- Keep: `on_new_window` denied, downloads denied, `gateway-*` permission
  policy (PR #389) denying IPC and media by default.

**Gate:** a real site loads; it cannot reach `/v1/submit`; IPC is denied;
`getUserMedia` still raises native consent.

## PR-4 — Injected wallet

**Depends on PR-0, PR-1, PR-3.**

- CEF init script: EIP-1193 provider + EIP-6963 announce, carrying a token
  bound to `(webview, origin)`.
- Wallet bridge routes on PR-0's listener: RPC passthrough (`eth_call`,
  `eth_getBalance`, `eth_estimateGas`, `eth_chainId`, …).
- `eth_requestAccounts` → native connect prompt; unapproved origins see zero
  accounts.
- `wallet_sendCalls` / `wallet_getCallsStatus` (EIP-5792) — the correct path.
- `eth_sendTransaction` → SafeTxHash, plus the `SafeTxHash -> real tx hash`
  mapping so `eth_getTransactionReceipt(safeTxHash)` returns the real receipt.
  This is what makes unmodified dapps work.
- `personal_sign` / `eth_signTypedData_v4` → ERC-1271 packed signature.

**Gate:** unapproved origin sees no accounts; EIP-5792 status transitions;
receipt aliasing resolves post-execution; ERC-1271 validates against a Safe on
anvil.

## PR-5 — Approval UX

**Depends on PR-4.**

- Native approval window (reuse PR #389's consent-window pattern — native
  chrome, unspoofable by a page). Decoded target, value, calldata.
- Vault console view: vaults, pending proposals, approvals, execute.
- Execute shows the ETH the executor needs (design §8: the threshold-crossing
  approver pays gas from their own EOA).

**Gate (campaign acceptance):** on the fleet, two seeded accounts own a 2-of-2
Safe on anvil; a dapp in the browser calls `eth_sendTransaction`; both owners
approve; the transaction lands and the dapp's receipt poll resolves.

## Sequencing

```
PR-0 ──┬── PR-3 ──┐
       │          ├── PR-4 ── PR-5
PR-1 ──┴── PR-2 ──┘
```

## Deliberately not built

Bitcoin/PSBT. Threshold ECDSA (the seam is an enum field on the vault record
plus the `safe.rs` boundary; no `trait VaultSigner` with one implementor).
Safe deployment from the console. Gas refunds from vault funds. Hardware-wallet
owners.
