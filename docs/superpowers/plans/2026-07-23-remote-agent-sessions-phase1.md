# Remote Agent Sessions Phase 1 — Credential Registry + Co-hosted Gateway

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Named, grantable, owner-hosted API credentials: `ducktape user cred add claude` registers `alice-claude-1` on-chain, `cred grant` lends it, and any node's broker can resolve the name and complete an Anthropic round-trip through the owner's co-hosted gateway without ever holding the secret.

**Architecture:** Consensus registry = credential records on the EXISTING gateway module (name → owner account, publisher node, kind, seal_pk, grants) — an app-hash flag day handled with the module-dev flow. Serving = the existing in-process airlock gateway grown from one in-memory credential slot to a named, disk-backed store with a no-TEE self-host mode whose trust anchor is the on-chain seal_pk. Transport = the existing `Service::Gateway` overlay plane via `Gateway::remote(handle, via)` — nothing new. CLI = a `user cred` subfamily; `cred add` wraps the vendor's own login CLI on a local pty writing directly into the gateway store.

**Tech Stack:** Rust; clap; axum (airlock server); existing crates only — no new dependencies.

**Spec:** `docs/superpowers/specs/2026-07-23-remote-agent-sessions-design.md`

## Global Constraints

- Work in a worktree at `<primary>/.worktree/remote-agent-cred-phase1`, branch `feat/remote-agent-cred-phase1` off `origin/dev`; deliver as PR(s) against `dev`. Create it with the superpowers:using-git-worktrees skill before Task 1.
- Invoke the repo `module-dev` skill before Task 1 and Task 2 (gateway module state change = app-hash flag day; wasm guest regen; parity fixtures).
- Lint gate per touched crate: `cargo clippy -p <crate> --tests --no-deps`. Format only code you touched; never `cargo fmt --all`.
- `tracing` only in node/daemon code — never `println!`/`eprintln!`. CLI stdout output (verb results, login URLs) IS program output and stays `println!`. Never log URI paths/query strings or key material; `reason` fields are snake_case tokens.
- No versioned names anywhere: no `v2` in types, routes, or protocol fields (repo mandate).
- Tests synchronize on events (channel recv, HTTP response, stream frame) — never sleep/spin.
- Credential names: lowercase `[a-z0-9-]`, 1–64 chars, must not collide with an existing record (first registration in consensus order wins).
- The two completion files `ops/completions/ducktape.bash` and `ops/completions/ducktape.zsh` must both carry every new verb token and long flag, or `completion_files_cover_the_verb_table` (`bin/node/src/cli.rs:1457`) fails.
- Commits end with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.

---

### Task 1: Gateway module — credential records (wire + state + arms)

**Files:**
- Modify: `crates/modules/system/gateway/src/interface.rs` (msg enum ~:196, query ~:208, reply ~:227)
- Modify: `crates/modules/system/gateway/src/registry.rs` (state ~:16, `root_of` ~:247)
- Modify: `crates/modules/system/gateway/src/module.rs` (`execute` ~:293, `query` ~:305)
- Test: `crates/modules/system/gateway/src/` unit tests alongside the existing module tests

**Interfaces:**
- Produces (later tasks depend on these exact names):
  - `pub struct CredentialRecord { pub name: String, pub owner_account: Vec<u8>, pub publisher_node: Vec<u8>, pub kind: CredentialKind, pub seal_pk: [u8; 32], pub grants: BTreeSet<Vec<u8>> }`
  - `pub enum CredentialKind { Claude, Codex }` (serde snake_case)
  - `GatewayMsg::{SetCredential, RemoveCredential, GrantCredential, RevokeCredential}`
  - `GatewayQuery::{Credential { name: String }, Credentials {}}`
  - `GatewayReply::{Credential(Option<CredentialRecord>), Credentials(Vec<CredentialRecord>)}`
  - Helper for later tasks: `pub fn credential_use_allowed(record: &CredentialRecord, account: &[u8]) -> bool` (owner or granted)

- [ ] **Step 1: Read the existing SetRoute shape end-to-end.** Read `interface.rs` (`GatewayMsg::SetRoute`, its signed statement type, the validation helpers) and the `execute` SetRoute arm in `module.rs`. The credential messages MUST mirror that exact authority pattern: an owner-signed statement embedded in the message, validated the same way SetRoute validates its publisher signature. Copy the statement/verify shape 1:1 — do not invent a second signing scheme.

- [ ] **Step 2: Write failing wire round-trip + validation tests.** In the gateway module's existing test module, following its existing test style:

```rust
#[test]
fn credential_wire_round_trips() {
    let record = sample_credential_record("alice-claude-1");
    let msg = signed_set_credential(&owner_key(), &record);
    let decoded = crate::decode(&crate::encode(&msg)).expect("decode");
    assert_eq!(msg, decoded);
}

#[test]
fn credential_names_are_validated() {
    for bad in ["", "UPPER", "has space", "x".repeat(65).as_str()] {
        assert!(validate_credential_name(bad).is_err(), "{bad:?} must be rejected");
    }
    assert!(validate_credential_name("alice-claude-1").is_ok());
}

#[test]
fn first_registration_wins_and_owner_gates_mutations() {
    let mut m = test_module();
    exec_ok(&mut m, signed_set_credential(&owner_key(), &sample_credential_record("a")));
    // duplicate name from another account: rejected
    exec_err(&mut m, signed_set_credential(&other_key(), &sample_credential_record("a")));
    // grant by non-owner: rejected; by owner: committed
    exec_err(&mut m, signed_grant(&other_key(), "a", other_account()));
    exec_ok(&mut m, signed_grant(&owner_key(), "a", other_account()));
    let rec = query_credential(&m, "a").expect("record");
    assert!(credential_use_allowed(&rec, &other_account()));
    // revoke then remove, owner-signed
    exec_ok(&mut m, signed_revoke(&owner_key(), "a", other_account()));
    assert!(!credential_use_allowed(&query_credential(&m, "a").unwrap(), &other_account()));
    exec_ok(&mut m, signed_remove(&owner_key(), "a"));
    assert!(query_credential(&m, "a").is_none());
}
```

Build the `signed_*`/`exec_ok`/`exec_err`/`query_credential` helpers on the module's existing test fixtures (the SetRoute tests already construct signed messages and drive `execute` — reuse those constructors).

- [ ] **Step 3: Run to verify failure.** `cargo test -p gateway credential` → FAIL (types not defined).

- [ ] **Step 4: Implement wire types.** In `interface.rs`, next to the route types:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialKind { Claude, Codex }

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CredentialRecord {
    pub name: String,
    pub owner_account: Vec<u8>,
    pub publisher_node: Vec<u8>,
    pub kind: CredentialKind,
    pub seal_pk: [u8; 32],
    pub grants: std::collections::BTreeSet<Vec<u8>>,
}

pub fn credential_use_allowed(record: &CredentialRecord, account: &[u8]) -> bool {
    let is_owner = record.owner_account == account;
    let is_grantee = record.grants.contains(account);
    is_owner || is_grantee
}

pub fn validate_credential_name(name: &str) -> Result<(), String> {
    let len_ok = (1..=64).contains(&name.len());
    let charset_ok = name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !(len_ok && charset_ok) {
        return Err(format!("credential name must be 1-64 chars of [a-z0-9-]"));
    }
    Ok(())
}
```

Add the four `GatewayMsg` variants and two `GatewayQuery`/`GatewayReply` variants, each carrying the SAME signed-statement wrapper the SetRoute family uses (exact field names copied from the existing statement type). `grants` on `SetCredential` starts empty; `GrantCredential`/`RevokeCredential` carry `{ name, account }` inside the signed statement.

- [ ] **Step 5: Implement state + arms.** `registry.rs`: add `credentials: BTreeMap<String, CredentialRecord>` beside `routes` in `State`, fold it into `root_of` the same way routes are folded (order-stable iteration). `module.rs` `execute`: one arm per new variant, each a single delegation to a named handler (`set_credential`, `remove_credential`, `grant_credential`, `revoke_credential`) — no `_` wildcard, no logic in arms (house rule). Handlers: verify the owner signature exactly as the SetRoute handler does, then `validate_credential_name`, then first-wins/owner-gate checks as named predicates:

```rust
fn set_credential(&mut self, stmt: SignedCredentialStatement) -> Result<(), String> {
    verify_credential_statement(&stmt)?; // same primitive the SetRoute arm uses
    validate_credential_name(&stmt.record.name)?;
    let taken_by_other = self
        .registry
        .credential(&stmt.record.name)
        .is_some_and(|existing| existing.owner_account != stmt.record.owner_account);
    if taken_by_other {
        return Err("credential name already registered".into());
    }
    self.registry.stage_credential(stmt.record);
    Ok(())
}
```

`query` arms return `GatewayReply::Credential` / `Credentials` from committed state.

- [ ] **Step 6: Run tests.** `cargo test -p gateway` → all PASS (new + existing).

- [ ] **Step 7: Lint + commit.**

```bash
cargo clippy -p gateway --tests --no-deps
git add crates/modules/system/gateway
git commit -m "feat(gateway): named credential records with owner-signed grants"
```

---

### Task 2: Flag day — schema fingerprint, wasm guest, parity

**Files:**
- Modify: `bin/node/src/constants.rs` (`MODULE_STATE_SCHEMAS` ~:145 — bump the `gateway` entry's u32)
- Regenerate: `crates/modules/system/gateway` guest wasm via `make wasm-modules` (updates the node artifact + `crates/kernel/host/tests/fixtures/gateway.component.wasm`)
- Test: existing `genesis_registry_matches_module_ids` (`bin/node/src/host_state.rs:1281`), gateway wasm parity test under `crates/kernel/host/tests/`

**Interfaces:**
- Consumes: Task 1's committed module change.
- Produces: a consistent app-hash across native and wasm; later tasks build on a tree where `make wasm-modules-check` is green.

- [ ] **Step 1: Bump the gateway schema version.** In `MODULE_STATE_SCHEMAS`, increment ONLY the `("gateway", n)` entry to `n + 1`. Do not touch the other 19.

- [ ] **Step 2: Regenerate the guest.** `make wasm-modules`. WASM REGEN TRAP: the module is embedded via `include_bytes!` — a consensus change is INERT until this rebuild. If the build machine lacks the builder toolchain, stop and surface it instead of skipping.

- [ ] **Step 3: Run the parity gates.**

```bash
make wasm-modules-check
cargo test -p node genesis_registry_matches_module_ids
cargo test -p host
```

Expected: PASS. If a fixture `cmp` fails, the regenerated artifact wasn't copied into the fixtures — rerun `make wasm-modules`, don't hand-edit fixtures.

- [ ] **Step 4: Commit.**

```bash
git add bin/node/src/constants.rs crates/kernel/host/tests/fixtures crates/guests
git commit -m "feat(gateway)!: credential records schema — app-hash flag day + wasm regen"
```

---

### Task 3: Airlock server — named multi-credential store + self-host mode

**Files:**
- Modify: `crates/modules/system/airlock/src/server.rs` (`GatewayConfig` :33, `AppState` :57, routes :168, `credential` :257, `session` :297, `proxy` :326, `refresh_now` :527)
- Modify: `crates/modules/system/airlock/src/wire.rs` (`SessionRequest` :40)
- Test: airlock's existing server tests (testkit feature)

**Interfaces:**
- Consumes: nothing from Tasks 1–2 (pure library change; the name strings meet in Task 5).
- Produces:
  - `GatewayConfig` gains `pub attest: AttestMode` where `pub enum AttestMode { Tsm(String), SelfHost }` (replaces the `attest: String` field; `Tsm` carries the old `tdx|snp|auto` string).
  - `pub fn build_seeded(cfg: GatewayConfig, seeds: Vec<(String, CredentialPayload)>) -> Result<(Router, String)>` — seed list replaces the single `Option`.
  - `SessionRequest.sub` (existing field) now carries the CREDENTIAL NAME; sessions, budgets, and refresh state are keyed per name.
  - In `SelfHost` mode `GET /attestation` returns `vendor: "self-host"` with an empty quote, and the seal keypair is the one passed in via `GatewayConfig` (new field `pub seal_keypair: Option<SealKeypair>`; `None` = generate, TEE path unchanged).

- [ ] **Step 1: Write failing tests** (testkit feature, mirroring the existing session/proxy tests):

```rust
#[tokio::test]
async fn sessions_route_to_the_named_credential() {
    // two seeds with distinct bearer tokens; a session opened with sub="a"
    // must proxy with credential a's token, sub="b" with b's.
    let app = build_seeded(self_host_cfg(), vec![
        ("a".into(), CredentialPayload::Bearer { access_token: "tok-a".into() }),
        ("b".into(), CredentialPayload::Bearer { access_token: "tok-b".into() }),
    ]).unwrap();
    let seen_a = round_trip_via(&app, "a").await; // helper: open session, POST /v1/messages to mock upstream, return the Authorization header the upstream saw
    let seen_b = round_trip_via(&app, "b").await;
    assert_eq!(seen_a, "Bearer tok-a");
    assert_eq!(seen_b, "Bearer tok-b");
}

#[tokio::test]
async fn unknown_credential_name_is_refused_at_session_open() {
    let app = build_seeded(self_host_cfg(), vec![]).unwrap();
    let status = open_session_status(&app, "missing").await;
    assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn self_host_attestation_reports_no_quote() {
    let app = build_seeded(self_host_cfg(), vec![]).unwrap();
    let att: AttestationResponse = get_attestation(&app).await;
    assert_eq!(att.vendor, "self-host");
    assert!(att.quote_b64.is_empty());
}
```

- [ ] **Step 2: Run to verify failure.** `cargo test -p airlock --features server,testkit` → FAIL.

- [ ] **Step 3: Implement.** `AppState`: replace `oauth: Mutex<Option<Oauth>>` with `creds: Mutex<HashMap<String, CredEntry>>` where `struct CredEntry { oauth: Oauth, refresh_gate: Arc<tokio::sync::Mutex<()>> }`; key `budgets`/`seen_nonces` by `(name, session)` instead of session alone. `session` handler: look up `req.sub` in `creds`, 404 with a snake_case reason body when absent. `proxy`/`refresh_now`: operate on the entry resolved from the session's name. `AttestMode::SelfHost`: skip the tsm quoter, serve the empty-quote attestation, use the injected `seal_keypair`. Keep the TEE arm byte-identical in behavior — one `match` on `AttestMode` at each divergence point, no boolean flags.

- [ ] **Step 4: Run tests.** `cargo test -p airlock --features server,testkit,client,verify` → PASS (new + existing; existing single-credential tests update to the seed-list signature).

- [ ] **Step 5: Lint + commit.**

```bash
cargo clippy -p airlock --tests --no-deps
git add crates/modules/system/airlock
git commit -m "feat(airlock): named multi-credential store and self-host attest mode"
```

---

### Task 4: Node co-host — disk-backed credential store, seal keypair, boot wiring

**Files:**
- Modify: `bin/node/src/airlock_serve.rs` (`AirlockServe` :36, `from_env` :53, `resolve_credential` :87)
- Modify: `bin/node/src/boot/surfaces.rs` (:108-144 — `build_seeded` call site :121)
- Test: `bin/node/src/airlock_serve.rs` unit tests + a boot-surface test beside the existing ones

**Interfaces:**
- Consumes: Task 3's `build_seeded(cfg, seeds)` + `AttestMode` + `GatewayConfig.seal_keypair`.
- Produces (Tasks 5–6 depend on these):
  - Store layout under the workspace: `<storage>/airlock-creds/seal.key` (32-byte seal secret, 0600), `<storage>/airlock-creds/<name>/` one dir per credential holding the vendor login artifact (`.credentials.json` for claude, `auth.json` for codex) plus `kind` (one line: `claude`|`codex`).
  - `pub fn cred_store_root(storage: &Path) -> PathBuf` and `pub fn load_seeds(root: &Path) -> Result<Vec<(String, CredentialPayload)>, String>` in `airlock_serve.rs`.
  - `pub fn load_or_create_seal_keypair(root: &Path) -> Result<SealKeypair, String>` — creates on first boot; the PUBLIC key is what `cred add` (Task 6) puts on-chain.
  - Self-host serving is on whenever the store root exists and is non-empty OR `DUCKTAPE_AIRLOCK_SERVE` is set (existing TEE env path unchanged and takes precedence).

- [ ] **Step 1: Write failing tests** for `load_seeds` (claude dir with a minimal `claudeAiOauth` credentials file → one `CredentialPayload::Refresh`; codex dir with `auth.json` access_token → `Bearer`; empty root → empty vec; a dir missing its artifact → skipped with a `tracing` warn, not an error) and `load_or_create_seal_keypair` (created once, stable across calls, file mode 0600).

- [ ] **Step 2: Run to verify failure**, then implement. `load_seeds` reuses the exact parse logic `resolve_credential` (:87-114) already has for `.credentials.json`; codex parse mirrors the broker's `auth.json` read (`capability-host/src/broker.rs:87-108` shape: `access_token` field). Boot wiring in `surfaces.rs`: build the seed list from the store, pass `AttestMode::SelfHost` + the loaded keypair when serving from the store, keep registering the single `RouteName::named("airlock")` route exactly as today (:123) — one route per node serves all its named credentials.

- [ ] **Step 3: Run.** `cargo test -p node airlock` → PASS.

- [ ] **Step 4: Lint + commit.**

```bash
cargo clippy -p node --tests --no-deps
git add bin/node/src/airlock_serve.rs bin/node/src/boot/surfaces.rs
git commit -m "feat(node): disk-backed named airlock credential store with self-host serving"
```

---

### Task 5: Broker — resolve a credential name, self-host trust path

**Files:**
- Modify: `crates/modules/system/capability-host/src/broker.rs` (`AirlockGateway` :1032, `AirlockConfig` :1043, `from_env` :1064, `AnthropicAuth::airlock` :997, `verify_gateway` :1120, `resolve_anthropic_upstream` :1172)
- Modify: `crates/modules/system/capability-host/src/lib.rs` (`RunAuth` :483 area — thread the resolved config)
- Test: broker unit tests beside the existing airlock tests

**Interfaces:**
- Consumes: Task 3's self-host gateway semantics (`sub` = credential name, empty-quote attestation).
- Produces (Task 7/e2e and Phase 2 depend on these):
  - `pub struct ResolvedCredential { pub name: String, pub kind: CredentialKind, pub authority: String, pub via: String, pub seal_pk: [u8; 32] }` in capability-host (its OWN `CredentialKind` mirror enum — capability-host must not depend on the gateway module crate; the node maps between them).
  - `AirlockConfig::self_host(resolved: &ResolvedCredential) -> AirlockConfig` — programmatic constructor: `gateway: Remote { handle: resolved.authority, via: resolved.via }`, `sub: resolved.name`, and a new `trust: AirlockTrust` field where `pub enum AirlockTrust { Attested { measurement: String, attest: String }, PinnedSealPk([u8; 32]) }` (replaces the loose `measurement`/`attest` fields; `from_env` builds `Attested`).
  - `RunAuth` (or the narrowest existing seam that reaches broker construction) gains `pub airlock: Option<AirlockConfig>`; when set it takes precedence over `AirlockConfig::from_env()` in `resolve_anthropic_upstream`.

- [ ] **Step 1: Write failing tests:**

```rust
#[tokio::test]
async fn pinned_seal_pk_skips_quote_verification_and_seals_to_the_pin() {
    // testkit gateway in self-host mode; broker configured with
    // AirlockTrust::PinnedSealPk(gateway_seal_pk). Session opens, one
    // sealed round-trip succeeds against the mock upstream.
}

#[tokio::test]
async fn pinned_seal_pk_mismatch_refuses_the_gateway() {
    // PinnedSealPk([0u8;32]) against the same gateway: session setup must
    // error BEFORE any credentialed request is sent.
}

#[test]
fn explicit_airlock_config_beats_env() {
    // RunAuth.airlock = Some(cfg) → resolve picks it even with
    // DUCKTAPE_AIRLOCK_* unset/absent (no env reads on this path).
}
```

- [ ] **Step 2: Run to verify failure**, then implement. In `AnthropicAuth::airlock` the divergence is ONE `match` on `trust`: `Attested` runs today's `verify_gateway` path unchanged; `PinnedSealPk(pk)` skips quote verification and uses `pk` directly as the seal target for `open_session_sealed`. Mismatch detection: the handshake against a gateway whose real seal key differs fails sealed-open — surface that as a named error (`gateway_seal_pk_mismatch`). Config precedence in `resolve_anthropic_upstream`: explicit `RunAuth.airlock` first, then `from_env`, then host credential — the lib still never reads env outside the existing `from_env` boundary (repo rule: config parses once at the binary boundary).

- [ ] **Step 3: Run.** `cargo test -p capability-host --features <the existing airlock test features>` → PASS.

- [ ] **Step 4: Lint + commit.**

```bash
cargo clippy -p capability-host --tests --no-deps
git add crates/modules/system/capability-host
git commit -m "feat(capability-host): per-run airlock config with pinned-seal-pk self-host trust"
```

---

### Task 6: `user cred` CLI family

**Files:**
- Modify: `bin/node/src/userkey_cli.rs` (`UserCmd` :26, `run` :256)
- Create: `bin/node/src/cred_cli.rs` (the subfamily's args + verbs; keep `userkey_cli.rs` from growing — mono-file rule)
- Modify: `bin/node/src/main.rs` (module decl), `crates/modules/system/capability-host/src/interactive.rs` (:66 — make a public local-spawn constructor)
- Modify: `ops/completions/ducktape.bash`, `ops/completions/ducktape.zsh`
- Test: clap tree tests (`bin/node/src/cli.rs:1457` drift guard now covers the new tokens), unit tests in `cred_cli.rs`

**Interfaces:**
- Consumes: Task 1 wire types (`GatewayMsg` credential variants, `GatewayQuery::Credentials`), Task 4 store layout (`cred_store_root`, `load_or_create_seal_keypair`), the signing helpers `load_user_signer` (`userkey_cli.rs:414`) + the statement-signing primitive the gateway route verb uses (`user_sign_gateway_route` :715), submission via `reqwest::blocking` POST to `{node}/v1/submit` with `target: "gateway"` (the `redeem-invite` pattern, :846), node HTTP base resolution via `redeem_node` (:789).
- Produces:
  - `UserCmd::Cred(cred_cli::CredArgs)`; `CredArgs` holds `#[command(subcommand)] cmd: CredCmd` plus the shared `--node <url>` / `-n <chain-id>` pair (same shape as `RedeemArgs`).
  - `pub enum CredCmd { Add { provider: ProviderArg, name: Option<String> }, List { json: bool }, Remove { name: String }, Grant { name: String, account: String }, Revoke { name: String, account: String } }` with `#[derive(clap::ValueEnum)] pub enum ProviderArg { Claude, Codex }`.
  - In capability-host: `impl InteractiveSession { pub fn spawn_local(command: tokio::process::Command) -> Result<Self, String> }` — delegates to `spawn_on_pty(command, None, None, None, None)`. Doc comment states the ONLY intended caller: host-side vendor-login wrapping on the operator's own box (no sandbox because it is the operator's own credential and machine).
- `grant`/`revoke` `<account>` accepts a display_name (resolved via `IdentityQuery::Accounts`→ match on `display_name`, error listing candidates on ambiguity) or a hex account id.

- [ ] **Step 1: Write failing unit tests** in `cred_cli.rs` for the pure pieces:

```rust
#[test]
fn default_name_is_display_provider_counter() {
    let existing = ["alice-claude-1", "alice-claude-2", "alice-codex-1"];
    assert_eq!(derive_default_name("alice", ProviderArg::Claude, &existing), "alice-claude-3");
    assert_eq!(derive_default_name("alice", ProviderArg::Codex, &existing), "alice-codex-2");
    assert_eq!(derive_default_name("jess", ProviderArg::Claude, &[]), "jess-claude-1");
}

#[test]
fn login_stream_url_extraction() {
    let chunk = b"Visit the following URL to authorize:\n  https://claude.ai/oauth/authorize?code=abc\nthen paste the code.";
    assert_eq!(
        extract_auth_url(chunk),
        Some("https://claude.ai/oauth/authorize?code=abc".to_string())
    );
    assert_eq!(extract_auth_url(b"no url here"), None);
}
```

- [ ] **Step 2: Run to verify failure**, then implement the verbs:
  - `list`: query `{node}/v1/query` HTTP lane with `gateway::encode_query(&GatewayQuery::Credentials {})`, decode, print a name/kind/owner/grants table; `--json` prints the records as JSON.
  - `remove`/`grant`/`revoke`: build the owner-signed statement with `load_user_signer` + the same statement-sign primitive Task 1 mirrored from SetRoute, POST `{node}/v1/submit` `{ target: "gateway", payload }`, print the committed height (redeem-invite pattern).
  - `add`:
    1. Preflight: `which::<binary>` via `std::process::Command::new(bin).arg("--version").output()` — on failure print `install {bin} first ({url})` and exit nonzero. Binary = `claude` / `codex` per provider.
    2. Resolve default name (`IdentityQuery::OfMember` with the user key → `AccountView.display_name`, fall back to error asking for an explicit name when unset; existing names from the `list` query).
    3. Create `<store>/<name>/`, run the vendor login on a local pty: `claude setup-token` with `CLAUDE_CONFIG_DIR=<dir>` / `codex login` with `CODEX_HOME=<dir>` via `InteractiveSession::spawn_local` inside a small `tokio::runtime::Runtime`. Pump: read chunks → scan with `extract_auth_url` → on first match print `open this url: <url>`; ALWAYS also mirror raw output to the terminal and forward stdin (fail-open — the wrap is presentation, not interception). Exit status ≠ 0 → abort without submitting.
    4. Verify the artifact landed (`.credentials.json` / `auth.json` exists in the dir), write the `kind` file.
    5. Read the seal PUBLIC key via `load_or_create_seal_keypair(cred_store_root(..))`, build `CredentialRecord { name, kind, seal_pk, owner_account, publisher_node }`, sign, submit, print `registered <name> at height <h>`.
  - Wire `UserCmd::Cred` into `run` (:256) as one delegation arm.

- [ ] **Step 3: Update BOTH completion files.** Add `cred` to the user verb table and a nested arm (mirror the `key)` arm at bash :50) carrying `add list remove grant revoke` and the long flags `--json --node --network`.

- [ ] **Step 4: Run the gates.**

```bash
cargo test -p node completion_files_cover_the_verb_table
cargo test -p node the_clap_tree_is_internally_consistent
cargo test -p node cred
```

Expected: PASS.

- [ ] **Step 5: Lint + commit.**

```bash
cargo clippy -p node --tests --no-deps
cargo clippy -p capability-host --tests --no-deps
git add bin/node ops/completions crates/modules/system/capability-host
git commit -m "feat(cli): user cred family — vendor-login wrap, grant/revoke, completions"
```

---

### Task 7: Two-node e2e — named resolution through the overlay gateway plane

**Files:**
- Create: `bin/node/tests/cred_lending.rs` (or extend the existing real-socket cluster e2e harness file if one covers gateway-plane tests — check `bin/node/tests/` first and reuse its cluster fixture)
- Test: itself

**Interfaces:**
- Consumes: everything above; airlock testkit mock upstream (`bin/airlock-gateway` `mock-upstream` logic lives in `airlock::testkit` — use the library form).

- [ ] **Step 1: Write the test** against the existing two-node cluster fixture (the real-socket lane the qa skill names):

```rust
#[tokio::test]
async fn granted_credential_resolves_and_round_trips_across_nodes() {
    let cluster = two_node_cluster().await; // existing fixture
    let (owner, compute) = (cluster.node(0), cluster.node(1));

    // owner: seed a self-host credential store + register the record on-chain
    seed_cred_dir(owner.storage(), "owner-claude-1", bearer("tok-e2e"));
    owner.restart_surfaces().await; // picks up the store; or start the cluster after seeding
    submit_signed_set_credential(&owner, "owner-claude-1", seal_pk_of(owner.storage())).await;
    submit_signed_grant(&owner, "owner-claude-1", compute.account_id()).await;
    cluster.wait_committed().await; // event-driven: existing commit-watch helper

    // compute: resolve the name from committed state, run one sealed round-trip
    let record = query_credential(&compute, "owner-claude-1").await.expect("record");
    let resolved = resolved_credential_from(&record, &compute); // authority + via from the record
    let cfg = AirlockConfig::self_host(&resolved);
    let reply = broker_round_trip(cfg, &compute).await.expect("round-trip");
    assert_eq!(reply, mock_upstream_pong());

    // negative: an ungranted third ACCOUNT (fresh keypair, no grant) is
    // refused at the gateway — two nodes suffice, the account is what's gated
    let stranger = fresh_account_keypair();
    assert!(broker_round_trip_claiming(&record, &compute, stranger.account_id()).await.is_err());
}
```

Adjust helper names to the fixture's real API — the assertions and event-driven waits are the contract. The mock upstream is the airlock testkit's, wired as the owner gateway's `anthropic_base`.

- [ ] **Step 2: Gateway-side grant enforcement.** This test forces the one piece not yet wired: the co-hosted gateway checking grants. Implement in `airlock_serve`/`server.rs` seam: the node passes the gateway a grant-lookup closure (query its own committed gateway-module state for the record, check `credential_use_allowed(record, claimed_account)`); the session request carries the claimed account (the `SessionRequest` gains `pub account_b64: Option<String>` — populated by the broker from its RunContext identity). Session open on an ungranted account → 403 `credential_not_granted`.

- [ ] **Step 3: Run.** `cargo test -p node --test cred_lending` → PASS. Also rerun the full touched-crate suites: `cargo test -p gateway -p airlock -p capability-host`.

- [ ] **Step 4: Lint + commit.**

```bash
git add bin/node crates/modules/system/airlock
git commit -m "test(node): two-node granted-credential lending e2e; gateway-side grant gate"
```

---

### Task 8: Codex lane

**Files:**
- Modify: `crates/modules/system/airlock/src/server.rs` (upstream selection per credential kind), `crates/modules/system/airlock/src/wire.rs` (seed carries kind)
- Modify: `crates/modules/system/capability-host/src/broker.rs` (codex auth arm consuming an airlock session — mirror of `AnthropicAuth::Airlock` on the codex side, ~:68-108 area where codex auth lives)
- Test: airlock server test + broker test, codex-shaped

**Interfaces:**
- Consumes: Task 3's named store (entries gain `kind: CredentialKind`), Task 5's `ResolvedCredential.kind`.
- Produces: `CodexAuth::Airlock(AirlockSession)` (or the codex-side equivalent of the existing auth enum — read `broker.rs:60-110` and mirror the Anthropic arm's shape exactly); gateway `proxy` selects upstream base + auth header shape by the session credential's kind (`Claude` → `anthropic_base` + existing Bearer, `Codex` → new `openai_base` config field + Bearer with the stored access_token; refresh for codex is OUT of scope this task — bearer-only, refuse `Refresh` payloads for codex seeds with a clear error).

- [ ] **Step 1: Write failing tests:** a codex-kind seed round-trips through the gateway to a mock OpenAI upstream (Authorization header assertion, same shape as Task 3's test); a claude-kind session cannot reach the codex upstream and vice versa (kind mismatch at session open → 409 `credential_kind_mismatch` when the request's declared kind differs); broker codex arm completes a round-trip with `AirlockConfig::self_host`.

- [ ] **Step 2: Run to verify failure, implement, run to green.** `cargo test -p airlock --features server,testkit,client && cargo test -p capability-host`.

- [ ] **Step 3: Lint + commit.**

```bash
git add crates/modules/system/airlock crates/modules/system/capability-host
git commit -m "feat(airlock): codex lane — per-kind upstream and broker arm"
```

---

### Task 9: Live QA (manual, before merge)

- [ ] `ducktape user cred add claude` on this dev box: real `claude setup-token` wrap, URL surfaced, artifact lands in the store, record commits. (Requires a real browser hop — do it with the user or document the exact command for them.)
- [ ] One real Anthropic round-trip through the self-host gateway with the registered credential (PONG-style minimal message), confirming the OAuth REFRESH path: force a refresh by seeding an expired access token with a live refresh token and observe `refresh_now` succeed — the `ANTHROPIC_OAUTH_TOKEN_URL`/`CLIENT_ID` constants (`broker.rs:842-843`) and the server-side twins are `PENDING live validation`; this is the step that validates them. Record the finding in the PR body.
- [ ] `cred grant` to a second account + round-trip from a second node (the e2e's manual twin over real WAN if a second box is available; else note the sim-lane coverage and defer WAN to Phase 2 QA).

Merge policy per repo rules: high confidence + green gates → PR(s) to `dev`. The natural PR split is Tasks 1–2 (flag day), 3–5 (serving+broker), 6 (CLI), 7–8 (e2e+codex) — stack them if review size demands.
