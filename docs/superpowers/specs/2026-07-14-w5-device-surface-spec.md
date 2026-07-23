# W5 — Device/key management surface (Keybase half)

Date: 2026-07-14. Work item **W5** of the account/workspace-separation epic
(ledger: `docs/superpowers/specs/2026-07-14-account-workspace-separation-epic.md`,
§W5 authoritative; model `docs/adr/2026-07-14-account-node-access-model.mdx`).
Branch: `feat/w5-device-surface`, forked from `epic/account-workspace-separation`.

## Scope (from the ledger)

- Device list aggregating **per-network `BindNode` records**, rendered as
  **cached last-known state per network**, refreshed when a network is
  connected/switched to. No live cross-network queries (single-active premise).
- Device labels **ON-CHAIN**: a new label op in `crates/system/identity`, so a
  device's label is visible from the user's other devices on that network.
- Remote-unbind UI, **per-network** (`UnbindNode`). No bulk "remove everywhere".
- Recovery story surfaced (mnemonic reveal/restore already exists —
  `CustodyCard`). Key rotation OUT of scope.
- Self-contained panel; final placement reconciles on the epic branch (W1 owns
  the account home).

## What already exists (built on, not redone)

- `NetworkNodesCard` — the CONNECTED network's account nodes, standing chips,
  per-node `Unbind` (the only `user_sign_unbind` consumer). This is
  connected-only; it is **superseded** by the new cross-network `DevicePanel`.
- `DevicesCard` — the account's member keys (account keys of any scheme),
  link/enroll/remove. Member keys are account-global; nodes are per-network.
- `CustodyCard` — recovery-phrase reveal, password lock. Recovery already here.

## Decisions (micro-decisions recorded)

### D1 — "device" in the aggregated list = per-network `BindNode` record (node)

The ledger says "aggregating per-network `BindNode` records" and "device
labels" in the same breath. A member key is account-global (same key every
network); a **node** is the per-network mesh identity. The aggregated list is
therefore over **nodes**, and the on-chain label attaches to the **node**
record. (Member-key labels already exist and are set once at enrollment;
renaming those is not what the device list shows.)

### D2 — label op is ORIGIN-GATED, like `SetAccountName` (not member-signed)

`SetNodeLabel { node_key, label }` is accepted only from a submitting node that
is itself bound to the account, and only targets a node in the **same account**.
No signature, no nonce bump — exactly `SetAccountName`'s contract.

Why origin-gated over user-signed (`UnbindNode`'s member-cert model):
- A device label is **cosmetic display metadata**, not a capability or a
  security boundary. `SetAccountName` is the module's precedent for exactly this
  ("a bound node is user-trusted hardware"); the node label is its sibling.
- Origin-gating rides the existing local `/v1/submit` lane (the node stamps its
  own key as origin) — **no new `user_sign_*` verb, no nonce-drift dance** in
  the tauri shell. Lazy and consistent.
- Capability is sufficient for the story: you label your devices while
  connected to that network (your node is bound there), and can rename any of
  your account's nodes on that network, including an offline/lost one.
- Limitation accepted: you must be connected to network N (with your bound
  node) to set a label on N. That is exactly the single-active model — you
  can only *act* on the connected network anyway. Cached labels render offline.

### D3 — node storage reshaped in place (no-backcompat mandate)

`AccountRecord.nodes: BTreeSet<Vec<u8>>` → `BTreeMap<Vec<u8>, NodeMeta>` where
`NodeMeta { label: Option<String> }`. `AccountView.nodes: Vec<Vec<u8>>` →
`Vec<NodeView>` (`{ node_key, label }`). The label lives welded to its node —
no parallel list to desync (correctness at the auth-adjacent `account.nodes`
read the gateway does). Genesis root-hash moves; QA networks need re-seed.

### D4 — cross-network cache = localStorage, panel-local (no store slice)

`DevicePanel` captures the connected network's device rows to `localStorage`
(keyed by chain id, mirroring the `doc-tabs` scope-store) on every relevant
change, and renders the union of cached networks. The connected network's group
is LIVE (from `state.nodeUsers`/`members`/`residents`, always fresh) and
actionable (label + unbind); cached networks render read-only with a "switch to
manage" hint. Keeping the cache panel-local (not a store slice) keeps the diff
self-contained — no `resetNodeProjection`/snapshot/actions-interface churn.

## Conflict management (W6 also touches identity crate — avatar/bio)

W5's identity diff is tight and additive: one new op variant
(`SetNodeLabel`), one new view struct (`NodeView`), one reshaped field on the
node record. W6 adds account-level profile fields — a disjoint region of
`AccountRecord`/`AccountView`. Enum-variant + struct-field additions merge
mechanically.

## REQUIRED build step — regenerate the identity wasm component

`identity` is an **adapter-ported WASM module**: the real node loads it from
`crates/examples/identity-wasm/component.wasm` (committed, `include_bytes!` in
`bin/node/src/host_state.rs`), the wasm compiled FROM the native crate. So a
native-crate change is NOT live on a real node — and worse, desyncs the wire —
until the component is rebuilt. This is precisely the root-hash move the ledger
flags (re-seed QA nets).

Regen command (the identity block of `make wasm-modules`):

```
cd crates/examples/identity-wasm && cargo build --target wasm32-unknown-unknown --release
wasm-tools component new \
  crates/examples/identity-wasm/target/wasm32-unknown-unknown/release/identity_wasm.wasm \
  -o crates/examples/identity-wasm/component.wasm
cp crates/examples/identity-wasm/component.wasm \
  crates/kernel/host/tests/fixtures/identity.component.wasm
```

**Blocked on this headless box**, so the committed `component.wasm` is NOT
regenerated in this PR:
- `clang` is absent and unroot-installable (`blst`, a transitive C dep, needs
  it to compile to wasm32; `gcc` cannot target wasm).
- Intermittent rustc SIGSEGV on this box's dep graph (documented trap).

Consequences until regen (must happen on the epic branch / a clang build box):
- `wasm_identity_parity` and `identity_e2e` are RED (native `AccountView` shape
  vs. the stale wasm's old shape) — these gates ENFORCE the regen before merge.
- `gateway_plane`'s `AccountView` decode breaks on a real node until regen, so
  gateway route authority is affected network-wide. Flagged loudly for the epic
  integrator. The native op logic itself is proven: identity unit tests (25) and
  the single-node **simnode** e2e (which runs identity NATIVELY) are green.

## Non-goals

- Key rotation (own ADR).
- Live cross-network queries / concurrent nodes (W3/W4 deferred).
- Bulk "remove everywhere" unbind.
- A new signing verb (origin-gating avoids it — D2).
