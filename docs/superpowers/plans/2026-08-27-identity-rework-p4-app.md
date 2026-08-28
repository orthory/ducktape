# Identity rework phase 4 — app account plane Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The iced app can found an account, list/add/remove its keys, and every gateway caller has a way to mint the request PoP phase 3's `Owner` audience checks.

**Architecture:** The app already signs every user op as a frame through one session `Signer` (`app/src/backend/rpc.rs`). Phase 4 adds four identity ops on top of it (`Create`, `AddKey` ticket minting, `AddKey` ticket joining, `RemoveKey`), a `chain_id` the node publishes in `/v1/status` (an `AddKey` consent is chain-scoped and the app had no chain id), the settings-card UI for them, and a CLI `user sign-caller` verb that prints the `x-duck-user-*` PoP for anything that talks to a gateway route (the app has no gateway/webview request site today — `duck://team.duck` is `DuckKind::Unknown` — so the stamping lives where a request can actually be made).

**Tech Stack:** Rust (iced app backend + Ice UI), noded `StatusCell`, clap CLI, bash/zsh completions.

**Spec:** `docs/superpowers/specs/2026-08-27-identity-rework-design.md` §App, §Request proof-of-possession, §Phases (4).

## Global Constraints

- No legacy/compat paths; no `OfNode`/`BindNode` anywhere (`grep -rn 'OfNode\|BindNode\|account_of_node\|node_is_current' crates bin app ops` stays empty).
- Ice: handlers are `match`/`let`/`slice` only (no `if`, no handler calls, no props); `none` is reserved; every writer of a mirrored view reading refreshes its mirror (`app/src/tests/stream.rs` lint).
- Gates: `cargo test -p ducktape-app`, `cargo test -p noded`, `cargo test -p node-bin --bin ducktape` (incl. `completion_files_match_the_clap_tree_per_family`), `cargo test -p simnode`, clippy `--tests --no-deps` on each; `rustfmt --edition 2024` on touched files only.
- The session signer is the only thing that touches the user key; every new op goes through `signed_write` or a sibling that holds the same `SIGNER` lock.

---

### Task 1: `/v1/status` carries `chain_id`

**Files:**
- Modify: `crates/noded/src/lib.rs` (`NodeStatus`), `crates/noded/src/handle.rs` (`StatusCell`), `crates/noded/src/testkit.rs:285`, `bin/node/src/validator/run.rs:78`, `bin/node/src/main.rs:566`, `bin/simnode/src/lib.rs:1421`
- Modify: `app/src/backend/node.rs` (`NodeFacts`, `node_facts`), `app/src/ui/extern/backend.ice:252`, `app/src/ui/state/node.ice`, `app/src/ui/handlers/node.ice` (both status handlers)

**Interfaces:**
- Produces: `NodeStatus.chain_id: String` (the identity module's chain id; `""` when the daemon has none); `StatusCell::wire_chain_id(&self, chain_id: String)` (once at boot; `current()` overlays it); app `NodeFacts.chain_id`, Ice state `network_chain_id`.

- [ ] Add `pub chain_id: String` to `NodeStatus` (doc: "the chain id every chain-scoped user proof — an `AddKey` consent, a route statement — is minted for; `""` on a daemon that serves no chain").
- [ ] `StatusCellInner.chain_id: OnceLock<String>`; `wire_chain_id` sets it; `current()` overlays `status.chain_id = wired.clone()` when set.
- [ ] Construction sites: `validator/run.rs` and `testkit.rs` add `chain_id: String::new()` (the cell overlays); simnode sets `chain_id: self.chain_id.clone()` (its identity chain id); `main.rs` calls `status.wire_chain_id(identity_chain_id.clone())` right after the first publish.
- [ ] noded test: `a_wired_chain_id_survives_every_boundary_publish` — wire, publish a status with `chain_id: ""`, assert `current().chain_id == wired`.
- [ ] App: `NodeFacts.chain_id` read from `status["chain_id"]`; `backend.ice` `NodeFacts(..., chain_id:str)`; state `network_chain_id = ""`; both `node_facts_loaded` and `node_status_pushed` set `network_chain_id = next.chain_id`.
- [ ] Gates: `cargo test -p noded --lib handle`, `cargo test -p ducktape-app`.

### Task 2: app identity ops (backend)

**Files:**
- Modify: `app/src/backend/rpc.rs` (a consent signer beside `sign_frame`), `app/src/backend/node.rs` (`AccountData`, four ops), `app/src/ui/extern/backend.ice`

**Interfaces:**
- Produces:
  - `AccountKeyRow { scheme: String, pubkey: String (hex), label: String, added_at: i64 }`; `AccountData.key_rows: Vec<AccountKeyRow>` (keeps `keys` count).
  - `create_account(rpc, password, name) -> bool ! AppError` — `IdentityMsg::Create { name, scheme: Ed25519 }` via `signed_write`.
  - `mint_key_ticket(rpc, password, chain_id, pubkey_hex, label) -> String ! AppError` — hex-decode, `KeyScheme::Ed25519.pubkey_wellformed`, query `KeyGen`, `rpc::sign_add_key_consent(password, chain_id, new_key, generation)` → `Authorizer`, returns the one-line JSON `AddKey` ticket (same bytes `ducktape account key join` accepts).
  - `join_with_ticket(rpc, password, ticket) -> bool ! AppError` — decode, refuse a non-`AddKey`, `signed_write("identity", ticket bytes verbatim)`.
  - `remove_account_key(rpc, password, pubkey_hex) -> bool ! AppError` — `IdentityMsg::RemoveKey { key }`.
  - `rpc::sign_add_key_consent` holds the `SIGNER` lock exactly like `sign_frame` and calls `workspace_config::ed25519_authorizer(&signer.key, chain_id, KeyScheme::Ed25519, new_key, generation)`.
- [ ] Unit tests (`app/src/backend/node.rs` tests): `a_ticket_is_the_add_key_the_cli_accepts` (decode → `AddKey{scheme: Ed25519, label, authorizer.key == this key}`; consent verifies at the minted generation under `IDENTITY_ADD_KEY_NS`), `a_non_add_key_ticket_is_refused_before_any_signature`.

### Task 3: settings UI — Create form, keys list, add/join, remove

**Files:**
- Modify: `app/src/ui/state/node.ice`, `app/src/ui/handlers/roster.ice`, `app/src/ui/screens/settings.ice`, `app/src/ui/view.ice`, `app/src/ui/tests/app.ice`, `app/src/tests/shell.rs`

**Interfaces:**
- State: `account_key_rows:[AccountKeyRow] = []`, `account_create_draft = ""`, `account_key_draft = ""` (pasted pubkey hex), `account_key_label_draft = ""`, `account_ticket = ""` (minted, shown to copy), `account_join_draft = ""`, `account_busy = false`.
- Events → handlers (roster.ice): `account_create_draft_changed`, `account_create_submit` → `run every create_account(...) -> account_changed _ | account_op_failed _`; `account_key_draft_changed`, `account_key_label_draft_changed`, `account_key_add_submit` → `mint_key_ticket(connected_rpc, password, network_chain_id, ...) -> account_ticket_minted _ | account_op_failed _`; `account_join_draft_changed`, `account_key_join_submit` → `join_with_ticket -> account_changed _`; `account_key_remove(str)` → `remove_account_key -> account_changed _`. `account_changed` bumps `account_generation`, reloads the account, clears drafts; `account_op_failed` sets `error = cause.message`.
- Screen: with `!account_exists`: a "Create account" row (name input + button, disabled when `empty(password)` or `account_busy`). With `account_exists`: KEYS group — `for row in account_key_rows` → scheme badge, label or "(unlabeled)", short pubkey, `Remove` (disabled when `account_keys <= 1` or busy); "Add a device" (pubkey + label inputs → "Mint ticket"; when `!empty(account_ticket)` show it + "Copy ticket"); "Join with a ticket" (ticket input + "Join").
- [ ] shell.rs lints: the Create row is under `if !account_exists`; the Remove button's `disabled=` names `account_keys <= 1`; every new state writer that moves `account_number` refreshes `rooms` (stream.rs lint already enforces).
- [ ] Handler test: `account_changed` bumps `account_generation` and clears the four drafts.

### Task 4: CLI `user sign-caller` — the gateway request PoP

**Files:**
- Modify: `bin/node/src/userkey_cli.rs` (`CallerArgs`, `UserCmd::SignCaller`, `user_sign_caller`), `bin/node/src/cli_args.rs` docs if the family doc lists verbs, `ops/completions/ducktape.{bash,zsh}`

**Interfaces:**
- `ducktape user sign-caller --key <PATH> --publisher-node <HEX> --account <N> --route <NAME> --method <GET|POST|…> --path <PATH-AND-QUERY>` — stdin: password. Prints `{"key","ts","sig"}` (hex key/sig, decimal ts) = the `x-duck-user-key/-ts/-sig` headers `noded::gateway_http::user_pop_headers` reads; signed over `gateway::caller_pop_preimage(publisher_node, account, &RouteName, method, path, ts)` under `gateway::GATEWAY_CALLER_NS` (route name via `gateway::RouteName::named` / `apex` for `""`; method via the same parser the gateway wire uses).
- [ ] Test: the printed sig verifies with `KeyScheme::Ed25519.verify(key, GATEWAY_CALLER_NS, preimage, sig)`; a bad method is refused.
- [ ] Completions: add `sign-caller` to `user_verbs` (bash+zsh) and `--publisher-node --account --route` to `user_flags` (the drift test `completion_files_match_the_clap_tree_per_family` gates it).

### Task 5: gates, PR

- [ ] `cargo test -p noded`, `-p ducktape-app`, `-p node-bin --bin ducktape`, `-p simnode`; clippy `--tests --no-deps` on noded, ducktape-app, node-bin, simnode; `rustfmt --edition 2024 --check` on touched files.
- [ ] Commit, `gh pr create --base dev`, merge when green, `ops/worktree-clean.sh --yes`.
