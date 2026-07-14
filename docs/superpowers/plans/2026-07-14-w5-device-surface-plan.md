# W5 device surface — implementation plan

Spec: `docs/superpowers/specs/2026-07-14-w5-device-surface-spec.md`.

## Rust — `crates/system/identity`

1. `interface.rs`
   - `NodeView { node_key: Vec<u8>, label: Option<String> }` (serde).
   - `AccountView.nodes: Vec<Vec<u8>>` → `Vec<NodeView>`.
   - `IdentityMsg::SetNodeLabel { node_key: Vec<u8>, label: Option<String> }`.
   - Doc the origin-gating (D2). Codec roundtrip test covers the new variant.
2. `lib.rs`
   - `NodeMeta { label: Option<String> }`; `AccountRecord.nodes: BTreeMap<Vec<u8>, NodeMeta>`.
   - `encode_state`: per node emit `push_bytes(key)` + `push_opt_str(label)`.
   - `decode_nodes`: read label; min-bytes-per-node 9; return the map.
   - `node_index_of` / snapshot dup-check / commit iterate `nodes.keys()`.
   - `account_view`: map nodes → `NodeView`.
   - `bind_node`: `nodes.insert(origin, NodeMeta { label: None })`.
   - `set_node_label`: origin bound → account A; target `node_key` bound to A;
     set/clear label (empty trim clears, `MAX_LABEL_LEN` guard); NO nonce bump.
   - `execute` arm + header doc.
3. `tests.rs`
   - Fix `bind_creates_*` node assertion to `NodeView`.
   - New: label set/clear round-trips + visible via `Get`; unbound origin
     refused; cross-account target refused; label survives in snapshot install;
     label dropped on unbind.

## Rust — consumers (mechanical)

- `bin/node/src/gateway_plane.rs` (2 spots): `candidate.node_key` / `n.node_key`.
- `crates/kernel/host/tests/wasm_identity_parity.rs`: assert against `NodeView`.

## Frontend

4. `app/src/domain/identity-client.ts`
   - `NodeView` type; `AccountView.nodes: NodeView[]`.
   - `setNodeLabel(transport, { nodeKey, label, origin })` — plain submit,
     mirrors `setAccountName`.
5. `app/src/console/store/hydration.ts`
   - node loop: `node.node_key`; carry `label` into `nodeUsers[hex]`.
6. `app/src/console/store/state.ts`
   - `nodeUsers` value gains `label: string | null`.
   - `loadNetworkDevices()` / `saveNetworkDevices(chainId, entry)` localStorage
     helpers (mirror `doc-tabs`). No store slice.
7. `app/src/console/store/actions.ts`
   - `accountSetNodeLabel(nodeHex, label)` — mirrors `accountUnbindNode`.
8. `app/src/console/views/home/DevicePanel.tsx` (new, self-contained)
   - Cross-network aggregated device list; live+actionable connected group
     (label edit, unbind, standing chip); cached read-only groups; recovery
     pointer. Replaces `NetworkNodesCard` in `HomeView`.
   - Delete `NetworkNodesCard.tsx` + `.test.tsx`.

## Tests

- Rust module tests (authoritative for the op).
- `app/src/test/sim/account.test.tsx`: extend — `set_node_label` over bare wire,
  read back through the people slice.
- `app/src/console/views/home/DevicePanel.test.tsx`: renders live + cached
  groups, label edit + unbind wired to actions (mirrors NetworkNodesCard.test).
- Update AccountView fixtures across TS tests to the `NodeView` shape.

## Gates

- `touch` a .rs then `cargo clippy -p identity --tests --no-deps`.
- `cargo test -p identity`; `bin/node/tests/identity_e2e.rs` if touched.
- `cargo clippy -p node --tests --no-deps` (gateway_plane consumer).
- `cargo check -p files --no-default-features` unaffected (not touched).
- bun typecheck + vitest for touched suites.
- NOT `cargo fmt --all`. Box traps: `CARGO_INCREMENTAL=0 RUST_MIN_STACK=…` on
  rustc segv; `sccache --stop-server` / `RUSTC_WRAPPER=""` on wrapper trouble.

## Skips / re-seed

- Live desktop/fleet QA skipped — happens on the epic branch later.
- Module change moves genesis app-hash → existing QA networks need re-seed.
