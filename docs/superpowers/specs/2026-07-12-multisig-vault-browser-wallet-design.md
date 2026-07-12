# Multisig Vault + Browser Wallet — Design

**Status:** design approved in-session 2026-07-12; not yet planned into PRs
beyond the phasing below.
**Depends on:** CEF runtime (PR #384), principal-based permission policy
(PR #389). Assumes CEF is the shipped webview; the browser half is not
buildable on wry/WebKitGTK.
**Supersedes, for this campaign:** the MPC-first framing in
`docs/records/research/2026-07-11-network-threshold-vault.md`. That record's
crux (custody committee must be decoupled from the validator set, because
consensus rotates by teardown-respawn) still stands and still binds any future
threshold backend. What changes is the *first* backend: native on-chain
multisig, not threshold ECDSA. See §3.

## Goal

An account can custody real external-chain assets (Ethereum + EVM L2s) behind
an M-of-N multisig, and can spend them **from a web app running in the
Ducktape browser** — including web apps we do not control, on the public
internet — via an injected `window.ethereum`.

Two halves, and they are separable:

- **The vault**: M-of-N coordination of outbound transactions, ordered by
  consensus.
- **The wallet**: an EIP-1193 provider injected into browser content, whose
  "account" is the vault.

## 1. The load-bearing insight: the chain is the verifier

Ethereum's `ecrecover` verifies Safe owner signatures. Our consensus never
has to. It only **collects and orders** opaque 65-byte signatures and checks
that the *submitter* is a bound owner.

This is what keeps the design cheap, and it is worth stating plainly because
the obvious design does the opposite. `crates/system/identity/src/scheme.rs`
holds a deliberately **closed, versioned** `KeyKind` enum
(`Ed25519 | P256 | WebauthnP256`) and its own docs warn that adding
`Secp256k1` requires "a shim replicating commonware's `union_unique` signing
preimage", calling it a "correctness footgun" to hand-roll a curve's signing
format inside a consensus module. We do not add that variant. A vault owner's
secp256k1 key is **not a Ducktape member key** — it is a chain-facing key
whose only verifier is Ethereum. No identity flag day, no new curve in the
consensus-critical path.

## 2. Owner keys derive from the existing mnemonic

`bin/node/src/userkey.rs` establishes that the 24-word mnemonic **is** the
identity: `Mnemonic::from_entropy` / `to_entropy` (never BIP39's `to_seed`
PBKDF2 stretch), so the mnemonic's 32 bytes of entropy *are* the ed25519 seed.

An owner's secp256k1 key is derived from that same seed by HKDF with a
distinct domain-separation label. Consequences, all good:

- No new custody surface, no second backup, no new recovery story. Restoring
  the mnemonic restores the vault-owner key.
- The derivation is deterministic and node-local. The key never enters
  consensus; only its 20-byte address does.
- The same derived key gives each member a personal EOA for free (§7.3).

## 3. Backend: native multisig now, MPC behind the same op shape

**Shipping backend — Gnosis Safe.** The vault is a Safe contract at
`(chain_id, safe_address)`. Owners are ordinary EOAs (§2). Each owner signs
the EIP-712 SafeTx hash; anyone submits `execTransaction` with the M
concatenated signatures. Zero new cryptography, zero DKG, zero resharing, no
interactive ceremony. Ducktape supplies exactly what it is good at — total
order and non-equivocating coordination — and the chain enforces the
threshold. Membership change is one on-chain transaction, not a cryptographic
event.

**Later backend — threshold ECDSA.** Buys chain-side privacy (the chain sees
a plain EOA), cheaper gas (one signature), and chains with no multisig
primitive. Costs DKG, resharing on every membership change, `cggmp21`
(Paillier + range proofs), share custody at rest, identifiable-abort handling,
and the largest audit surface in the repo. Gated behind an actual requirement.

**The seam is an enum field, not a trait.** The vault record carries
`backend: Safe | ThresholdEcdsa`, and all Safe-specific transaction
construction lives in one `safe.rs`. The consensus ops are *already*
backend-agnostic in shape — propose an intent, collect contributions, emit a
finished transaction. Under MPC a "contribution" becomes a signing round
instead of a signature, and the finished transaction carries one signature
instead of M. The browser wallet, the approval UI, and the op set do not
change.

We deliberately do **not** write `trait VaultSigner` with a single
implementation. An interface with one implementor abstracts over nothing; the
seam that actually protects us is the op shape and the file boundary, and
those cost nothing today. If MPC lands, the trait extracts itself.

**Bitcoin is out of scope.** It has no Safe: native P2WSH/taproot multisig,
PSBT signature collection, UTXO selection, fee estimation, and change
management share no code with the EVM path — and `window.ethereum` cannot
express any of it, so the browser wallet could never reach that half. Roughly
doubles the backend for a half the wallet cannot use.

## 4. Consensus plane — `crates/apps/multisig`

Named `multisig`, not `vault`: `crates/apps/vaults` already exists and is a
*different thing* (client-sealed team secrets, opaque ciphertext, no keys of
its own). Overloading the name would be a semantics collision. The product
surface still calls it a Vault.

### State

```
Vault {
    vault_id, chain_id, safe_address: [u8; 20],
    owners: Vec<AccountId>, threshold: u8,
    backend: Safe | ThresholdEcdsa,
    next_nonce: u64,
}
OwnerEoa    { vault_id, account_id -> address: [u8; 20] }
TxProposal  { tx_id, vault_id, to, value, data, operation,
              safe_nonce, safe_tx_hash: [u8; 32], proposer }
Approval    { tx_id, account_id, contribution: Vec<u8> }   // Safe: a 65-byte sig
Execution   { tx_id, chain_tx_hash: [u8; 32], status }
```

`owners` and `threshold` are a **mirror**. The Safe contract is authoritative;
the mirror exists so the UI and the approval tally can work without an RPC
round-trip. It is advisory and must never override on-chain reality — an
`execTransaction` with M signatures either satisfies the *contract's*
threshold or reverts, whatever our mirror believed. Drift is detected by the
oracle (§5) and surfaced, not silently reconciled.

### Ops

- `RegisterVault` — records an existing Safe. (Deploying a new Safe is a
  chain transaction like any other; it is not a consensus op.)
- `BindOwnerEoa` — binds `account_id -> address`, proven by ecrecover over a
  chain-scoped preimage. This is the only place a secp256k1 verify enters
  consensus, and it is a standard `ecrecover`, not a hand-rolled preimage
  format.
- `ProposeTx` — the module **computes** `safe_tx_hash` itself and **allocates**
  `safe_nonce` itself. Both are load-bearing:
  - The SafeTx hash is a pure EIP-712 function of `(safe_address, chain_id,
    to, value, data, operation, nonce)`. Computing it in-module means a
    proposer cannot lie about what the other owners are being asked to sign.
  - The nonce is allocated monotonically per vault (`next_nonce`, seeded from
    the chain at registration). Two concurrent proposals otherwise take the
    same nonce and only one can ever execute. This is what Safe's own
    Transaction Service does; consensus is a strictly better nonce allocator
    than a centralized service.
- `Approve` — verified in consensus: ecrecover the signature over the
  proposal's `safe_tx_hash`, and the recovered address must equal that
  account's bound EOA. Pure, deterministic, no clock, no I/O. Crossing
  `threshold` emits an event: the transaction is executable.
- `RecordExecution` — written by the oracle after broadcast (§5).

New dependency: `k256` for `ecrecover` (already present transitively in the
revm tree).

### Determinism

Every op above is a pure function of committed state and op bytes. No RPC, no
clock, no randomness. All chain contact is quarantined in §5.

## 5. Chain I/O — saga effect + oracle worker

Reading the chain and broadcasting to it are non-deterministic external I/O
and cannot live in consensus. `crates/system/dispatch-oracle` is already
exactly this shape: a host-side worker resolves an effect, runs the impure
call, and submits the answer back as an ordered op. We reuse the pattern.

- **Read** (`eth_getTransactionCount` on the Safe, balances, owner set, code):
  used at `RegisterVault` to seed `next_nonce` and to detect mirror drift.
- **Broadcast**: on threshold, an effect carries the finished
  `execTransaction` calldata; the worker submits it via the configured RPC and
  reports the receipt back as `RecordExecution`.
- **Config**: RPC endpoint per `chain_id`, user-supplied in settings. No
  bundled provider key.

Foreign-lease skipping (the same reason dispatch-oracle does not have every
node pay for the same LLM call) applies: exactly one node broadcasts.

## 6. Opening the browser to the internet — the blocker

`app/src/domain/duck-browser.ts` rejects any host that is not
`<account>.duck`, and `gateway_window.rs` pins each gateway webview to one
`(scheme, host, port)` via `on_navigation`. Lifting those is a small change.

**It cannot be made without splitting the control-plane listener first.**

`bin/noded/src/lib.rs` serves `/v1/submit`, `/v1/query`, `/v1/files/*`,
`/v1/fs/*`, `/v1/gateway/*` and `/forge/*` from **one** axum router, with no
origin guard and permissive CORS. That was safe only because the trusted
console was the sole browser principal. The moment `https://evil.com` renders
in a CEF webview it can `fetch('http://127.0.0.1:<port>/v1/submit')` — and
`on_navigation` does not gate `fetch`, while CORS does not prevent the request
from *arriving*, only from being read. A hostile page would forge consensus
ops as the node, read all state, write files, and push git. The
gateway-v2 audit already flagged this as a BLOCKER for `duck://` alone; for
arbitrary web content it is fatal.

Required, and worth shipping on its own merits:

1. **Bearer-token auth on `/v1`.** The token lives at
   `~/.ducktape/<ws>/api.token`. Local processes (CLI, agents, `git push` to
   forge) read the file; **web content cannot read the filesystem**, so it can
   never present the token. A cross-origin `fetch` additionally cannot set an
   `Authorization` header without a preflight, which we fail. One middleware,
   one file — the Jupyter/Docker pattern.
2. **Strict CORS** replacing the permissive header.
3. **The wallet bridge gets its own listener**, exposing only wallet methods.
   `/v1/submit` is never reachable from a page, by construction rather than by
   policy.

Chromium's Private Network Access also blocks public→private subresource
requests, but its behavior is CEF-version-dependent: defense in depth, not the
defense.

Retained from today's gateway webviews: `on_new_window` denied, downloads
denied, and the `gateway-*` permission policy (PR #389) that denies IPC and
media by default with native consent for grants. Per-origin connect
permissions for the wallet are new (§7.4).

## 7. The wallet

Injected as a CEF initialization script into browser webviews. It speaks
**EIP-1193** and announces via **EIP-6963**.

### 7.1 Reads are a proxy

`eth_call`, `eth_getBalance`, `eth_estimateGas`, `eth_getLogs`,
`eth_blockNumber`, `eth_chainId` pass straight through to the configured RPC.
This is the bulk of provider traffic and involves no consensus at all.

### 7.2 The tx-hash problem, solved for real

`eth_accounts` returns the **Safe address**. But `eth_sendTransaction` must
return a transaction hash, and at that instant **no transaction exists**: it
needs M approvals, and the eventual on-chain hash depends on the executor's
nonce and gas. This is the wart that makes multisigs hang real dapps. Two
answers, shipped together:

- **EIP-5792** (`wallet_sendCalls` / `wallet_getCallsStatus`) is the standard
  built for exactly this — asynchronous smart-account execution. wagmi already
  speaks it. This is the correct path and the one we document.
- **For legacy dapps**, `eth_sendTransaction` returns the SafeTxHash, and the
  bridge retains the `SafeTxHash -> real tx hash` mapping so that
  `eth_getTransactionReceipt(safeTxHash)` answers with the **real receipt**
  once executed. The dapp polls a key it does not know is synthetic and gets a
  true answer. This is what makes an unmodified dapp work against the vault.

### 7.3 Signatures

`personal_sign` / `eth_signTypedData_v4` collect M owner signatures and return
an **ERC-1271** packed signature, verified on-chain by the Safe's
`isValidSignature`. Dapps that call `ecrecover` directly will reject it. That
is true of every smart-contract wallet, is not fixable, and is documented
rather than worked around.

The member's **personal EOA** (§2, same derived key) is also exposed as a
selectable account. It costs nothing and makes dapp *login* work on the many
sites that never implemented ERC-1271.

### 7.4 Transport and origin trust

The provider talks to the **wallet bridge listener** (§6.3), never to Tauri
IPC — PR #389 correctly denies IPC to `gateway-*` webviews, and handing
arbitrary web content Tauri IPC would be remote code execution.

Each webview's init script carries a random token bound to
`(webview, origin)`; the bridge checks the token against the request's
`Origin`. A page cannot read another origin's token (separate JS context and,
under site isolation, a separate renderer process).

Per-origin connect permission is explicit and persisted: an origin sees no
accounts until the user approves it, mirroring MetaMask. `eth_requestAccounts`
raises the native prompt; `eth_accounts` returns empty for an unapproved
origin.

### 7.5 Approval UX

Spending raises a **native approval window**, reusing PR #389's consent-window
pattern (native chrome, not web content — a page cannot spoof it). It shows
the decoded target, value, and calldata, and on approval signs the SafeTx hash
with the member's derived key and submits an `Approve` op. Other owners see
the pending proposal in the Vault console view and approve there.

## 8. Defaults taken (each is a cheap reversal)

- **Gas:** the approver who crosses the threshold executes, paying from their
  own derived EOA. Safe's `refundReceiver` could reimburse them from vault
  funds, but gas-refund parameters are a known griefing vector; the UI instead
  shows the ETH needed to execute. Revisit if funding friction is real.
- **RPC:** user-configured per `chain_id`. No bundled provider key.
- **Personal EOA exposed** alongside the vault (§7.3).

## 9. Testing

- **Pure/consensus:** SafeTx-hash EIP-712 vectors against Safe's published
  test vectors (this is the one place a hash mistake silently produces
  unspendable approvals); nonce allocation under concurrent proposals;
  `Approve` rejecting a signature from a non-owner, from an owner over the
  wrong hash, and replayed onto another proposal; ecrecover binding.
- **Oracle:** broadcast against a local anvil fork; receipt recorded; mirror
  drift detected.
- **Listener split (§6):** a page origin cannot reach `/v1/submit` without the
  token; a preflight for `Authorization` is refused; the CLI and `git push` to
  forge still work.
- **Wallet:** unapproved origin sees zero accounts; EIP-5792 status
  transitions; `eth_getTransactionReceipt(safeTxHash)` returns the real
  receipt post-execution; ERC-1271 signature validates against a deployed Safe
  on anvil.
- **End-to-end (fleet):** two seeded accounts own a 2-of-2 Safe on an anvil
  chain; a dapp in the browser calls `eth_sendTransaction`; both approve; the
  transaction lands and the dapp's receipt poll resolves.

## 10. Out of scope

Bitcoin (§3). Threshold ECDSA (§3). Deploying Safes from the console (use the
chain). Gas refunds from vault funds (§8). Hardware-wallet owners (the derived
key is the only owner kind at MVP). Public-web *gateway publishing* — the
browser reaching the internet is not the mesh serving the internet, and the
latter remains out of scope per gateway-v2 §"Reach is shell-only".
