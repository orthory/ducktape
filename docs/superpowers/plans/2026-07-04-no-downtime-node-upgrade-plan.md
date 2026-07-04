# No-Downtime Node Upgrade — Implementation Plan

## Goal

Implement the height-gated, dual-path node-upgrade mechanism specified in the
design of record so a `root()`/op-encoding change (module id, consensus scheme,
and genesis all unchanged) can be shipped to a live Ducktape network with zero
network downtime: a pre-shipped dual-path binary runs OLD logic byte-for-byte
below an agreed activation height `H`, every current boundary validator signals
readiness, and at `H` all ready validators flip to NEW logic together over the
existing epoch teardown-respawn boundary. Activation is **execute-time** — the
version bump, pending-clear, and every module migration are deterministic in-block
transitions keyed on `Env.height` and sealed into module roots, so a live node, a
recovery replay, and a state-sync joiner all reconstruct the activation
byte-for-byte. This plan lands the whole machinery as **inert scaffolding**
(compiling and tested) before the single coordinated PR that first registers the
module and moves the genesis app-hash.

Design of record (authoritative, read alongside this plan):
[`docs/superpowers/specs/2026-07-04-no-downtime-node-upgrade-design.md`](../specs/2026-07-04-no-downtime-node-upgrade-design.md).

## Scope and non-goals

### In scope
- A genesis-constant `upgrade` system module (`crates/system/upgrade` +
  `crates/system/upgrade-interface`) holding `current_version`, the single pending
  `Upgrade`, and the per-validator readiness set — all folded into the app-hash.
- `GovAction::ScheduleUpgrade` / `CancelUpgrade` authorizing (not arming) an
  upgrade via the existing member-gated simple-majority tally.
- The validator-origin `SignalReady` op and the `R = n` arming gate (with `2f+1`
  as the mathematical safety floor beneath it, never the arming policy).
- `effective_version(height)` — a **pure derivation** over committed state —
  threaded read-only through `BlockContext`/`Env` as `protocol_version`, and an
  internal `active_version` on each root()-changing module (forge is the worked
  example).
- Deterministic arm/abort/activate at `H` over the epoch teardown-respawn boundary
  (`ValsetOrchestrator`), plus recovery + state-sync manifest version fields and a
  fail-loud boot preflight.

### Out of scope (mirrors the spec Non-goals)
- **Module-SET changes** (adding/removing a module id). The base guarantee covers
  only `root()`/op-encoding changes with a stable module id. A set change needs
  height-gated registry composition reproduced identically across live,
  recovery-replay, and state-sync-install; that is a separate, larger design.
- **The Ducktape-2 retrofit.** Introducing the `upgrade` module onto an
  already-live network that lacks it is a one-time coordinated stop-the-world
  genesis bump, not a no-downtime upgrade. It sits outside the zero-downtime
  guarantee. (In THIS repo, Phase 8 is the equivalent coordinated genesis bump on
  `dev`; the separate live Ducktape-2 net stays on its old binary, untouched.)
- **Runtime binary distribution.** How operators fetch, verify, and stage the
  dual-path binary belongs to the upgrade skill/ops runbook, not the consensus
  mechanism. The mechanism only cares the correct binary is present per the
  readiness/preflight rules.
- **Downgrades / state rewinds.** Version is monotonic; recovery is roll-forward
  only (`ScheduleUpgrade` to `to_version + 1`).
- **Changing the upgrade module's own logic** — that would be its own separately
  versioned, height-gated upgrade.

### Guiding invariant for the landing order
Everything in Phases 1–7 is inert: the module is present-but-unregistered,
`protocol_version` defaults to a baseline and is **never hashed**, manifest fields
are additive with tolerant decode, and the new `GovAction` variants take fresh tag
bytes that nobody proposes yet. The running-network genesis app-hash does not move
until Phase 8, the single coordinated genesis bump.

---

## Phases

### Phase 1 — `upgrade` module + `upgrade-interface` crate (inert, UNREGISTERED)

Introduce the consensus state and its wire surface, modeled 1:1 on
governance/valset, but do **not** register it in any genesis vec yet.

#### Task 1.1 — Create `crates/system/upgrade-interface`
- **Files to create:** `crates/system/upgrade-interface/Cargo.toml`,
  `crates/system/upgrade-interface/src/lib.rs`.
- **Change:** the types-only public surface, plus the shared pure derivation.
  `encode_msg`/`decode_msg`/`encode_query`/`decode_reply` are serde_json, exactly
  like `governance-interface` (which rides `GovAction` inside `GovMsg` via
  serde_json).
- **Key types:**
  ```rust
  #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
  pub struct Upgrade { pub name: String, pub activation_height: u64, pub to_version: u32 }

  #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
  pub enum UpgradeMsg {
      Schedule { name: String, activation_height: u64, to_version: u32 }, // Origin::Module("governance")|System
      Cancel   { name: String },                                          // Origin::Module("governance")|System
      SignalReady { name: String, to_version: u32, commitment: Option<Vec<u8>> }, // Origin::External(pubkey)
      Advance, // host/system-injected boundary tick, keyed on env.height
  }
  pub enum UpgradeQuery { Status }
  pub enum UpgradeReply { Status(UpgradeStatus) }
  #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
  pub struct UpgradeStatus {
      pub current_version: u32, pub pending: Option<Upgrade>,
      pub ready: Vec<Vec<u8>>, pub member_count: u64, pub ready_count: u64, pub armed: bool,
  }
  pub fn encode_msg(m: &UpgradeMsg) -> Vec<u8>;
  pub fn decode_msg(b: &[u8]) -> Result<UpgradeMsg, String>;
  pub fn encode_query(q: &UpgradeQuery) -> Vec<u8>;
  pub fn decode_reply(b: &[u8]) -> Result<UpgradeReply, String>;

  /// pure, total, no IO/clock/RNG; identical on every node, live/replay/sync.
  pub fn effective_version(height: u64, current: u32, pending: Option<&Upgrade>, boundary_members: &[Vec<u8>], ready: &BTreeMap<Vec<u8>, ()>) -> u32;
  ```
  Note: `effective_version` here evaluates the *armed* predicate against the
  caller-supplied boundary member list and the readiness keys (per spec §"Version
  is a pure derivation"). The module re-exports the same predicate as a `&self`
  helper (Task 1.2) so orchestrator and module never hand-copy the logic (risk R4).
- **Test:** `cargo build -p upgrade-interface`; a serde round-trip unit test for
  every `UpgradeMsg`/`UpgradeQuery`/`UpgradeReply` variant; an `effective_version`
  truth table (pending None → current; armed & `height < activation_height` →
  current; armed & `height >= activation_height` & all boundary members ready →
  to_version; a boundary member missing → current).
- **Acceptance:** crate compiles; round-trip + truth-table tests green.

#### Task 1.2 — Create `crates/system/upgrade`
- **Files to create:** `crates/system/upgrade/Cargo.toml`,
  `crates/system/upgrade/src/lib.rs`.
- **Change:** the module implementing `Module`, mirroring governance's
  staged-over-committed seam, canonical encoding, and verify-then-adopt
  snapshot/install.
- **Key types:**
  ```rust
  struct ReadySignal { commitment: Option<Vec<u8>> } // idempotent per pubkey, last-write-wins
  struct UpgradeState {
      current_version: u32,
      pending: Option<Upgrade>,                       // at most one, ever
      readiness: BTreeMap<Vec<u8>, ReadySignal>,      // 32-byte ed25519 member pubkeys
  }
  pub struct Upgrade {
      id: ModuleId, valset_id: ModuleId,
      committed: UpgradeState, staged: Option<UpgradeState>,
  }
  impl Upgrade {
      pub fn new(id: impl Into<ModuleId>, valset_id: impl Into<ModuleId>) -> Self; // mirrors Governance::new
      /// pure derivation over COMMITTED state + caller-supplied frozen boundary set.
      pub fn effective_version(&self, height: u64, boundary_members: &[Vec<u8>]) -> u32;
  }
  const MIN_UPGRADE_LEAD: u64 = CUTOVER_DELAY; // >= orchestrator cutover_delay (=3, bin/node/src/main.rs:132)
  ```
  - **Staging seam** (mirrors valset/governance): `read()` =
    `staged.as_ref().unwrap_or(&committed)`; a mutate clones `read()` into
    `staged`, edits, stores; `commit_block` = `if let Some(s)=staged.take(){committed=s}`;
    `abort_block` = `staged=None`.
  - **`execute()` gates** (origin is authority, not the variant):
    - `Schedule`/`Cancel` accepted only from `Origin::Module("governance")` or
      `Origin::System`; `Origin::External` rejected with `Error::Module` (the exact
      valset origin gate, `crates/system/valset/src/lib.rs:240-247`).
    - `Schedule` validity: `to_version > current_version` (monotonic, no
      downgrade); `activation_height > env.height + MIN_UPGRADE_LEAD` (min-lead,
      never retroactive); `pending.is_none()` (at-most-one). A valid `Schedule`
      sets `pending` and clears any residual `readiness`.
    - `Cancel` valid only while `env.height < activation_height` and `name`
      matches; clears BOTH `pending` and `readiness`.
    - `SignalReady` accepted only from `Origin::External(pubkey)` where `pubkey` is
      a CURRENT valset member (host-routed `ValsetQuery::Validators` via
      `valset_id`); rejected from Module/System. Accepted only when `name` +
      `to_version` match the pending upgrade. Idempotent, keyed by pubkey. In
      phase 1 keep `commitment = None` recorded as presence only (risk R below).
    - `Advance` (system-injected boundary tick): evaluate the identical predicate
      as `effective_version` against committed state + the frozen boundary set; if
      armed (`R == n`) → `current_version = to_version`, clear `pending` +
      `readiness`; else (`H` reached, `R < n`) → clear `pending` + `readiness`
      (clean abort), leaving `current_version` unchanged. Idempotent no-op when no
      pending.
  - **Canonical encoding / root** (mirrors governance l.135-196):
    `encode_state(s)`: `current_version` u32-le; pending tag(0 none|1 some) +
    `push_bytes(name)` + `activation_height` u64-le + `to_version` u32-le;
    readiness `len` u64-le then per sorted `(pubkey, ReadySignal)`:
    `push_bytes(key)` + commitment tag + optional `push_bytes(commitment)`.
    `root_of(s)`: `StateRoot::ZERO` iff `current_version==0 && pending.is_none() &&
    readiness.is_empty()` (uninitialized sentinel), else
    `StateRoot(sha256(encode_state))`. `snapshot()` = `encode_state(&committed)`;
    `install(bytes, expected)` strictly decodes into a temporary (bounds-checked
    `take_u64`/`take_u8`/`take_vec` + strictly-increasing readiness keys, verbatim
    from governance l.403-511), recomputes `root_of`, refuses on mismatch, and only
    then sets `committed = decoded`, `staged = None`.
  - **`query_with(UpgradeQuery::Status)`:** reports `current_version`, `pending`,
    sorted `ready` keys, and `member_count`/`ready_count`/`armed` computed from a
    host-routed `ValsetQuery::Validators` read (`armed = pending.is_some() &&
    members non-empty && every member key ∈ readiness`).
- **Files to modify:**
  - `/home/eddy/dev/ducktape/Cargo.toml` — add `"crates/system/upgrade"`,
    `"crates/system/upgrade-interface"` to `[workspace].members` in the system
    block right after the governance entries (~l.25). Add two
    `[workspace.dependencies]` path entries: `upgrade = { path =
    "crates/system/upgrade" }` (module-impl block ~l.89), `upgrade-interface = {
    path = "crates/system/upgrade-interface" }` (interface block ~l.109).
- **Tests** (`crates/system/upgrade/src/lib.rs` `#[cfg(test)]`, using a stub `Ctx`
  that returns a fixed valset from `ValsetQuery::Validators`):
  - origin-gate: `Schedule`/`Cancel` from `Origin::External` → `Error::Module`;
    accepted from `Origin::Module("governance")` and `Origin::System`.
  - `SignalReady` origin-gate: rejected from Module/System; accepted from
    `Origin::External(pubkey)` only when a current member; a non-member signal is
    ignored/rejected.
  - `Schedule` validation: `to_version <= current_version` rejected;
    `activation_height <= env.height + MIN_UPGRADE_LEAD` rejected; a second
    `Schedule` while pending rejected; a valid `Schedule` sets pending and clears
    residual readiness.
  - `SignalReady` identity scope + idempotence: mismatched name/to_version
    ignored; two signals from one key collapse to one entry; N members → N entries.
  - `Cancel`: valid only while `env.height < activation_height` and name matches;
    clears both; mismatched/absent pending errors.
  - arm+activate at `H` (`Advance`): pending set, `height >= activation_height`,
    every boundary member signaled ⇒ `current_version = to_version`, cleared;
    `effective_version(H, members) == to_version` and equals the reconciled stored
    value.
  - clean abort at `H` (`Advance`): one member unsignaled ⇒ `current_version`
    unchanged, pending+readiness cleared; `effective_version` returns OLD version;
    a second `Advance` is an idempotent no-op.
  - readiness denominator is the BOUNDARY set: a member added after `Schedule` who
    has not signaled makes `R==n` unmet ⇒ abort; a signal from a key no longer a
    member is dead weight.
  - staging atomicity: `Schedule` staged then `abort_block` discards it — `root()`
    and committed bytes identical to pre-block; `commit_block` publishes staged and
    moves root off ZERO.
  - root discipline: fresh module ⇒ `root()==StateRoot::ZERO`;
    `sha256(snapshot())==root()` for non-empty state; snapshot/install round-trip
    reconstructs root+state; a bit-flipped/truncated/trailing snapshot is rejected
    and leaves committed+staged untouched.
  - query `Status`: reports fields + `armed=true` iff every member signaled.
- **Acceptance:** `cargo test -p upgrade` (full suite above) and `cargo test -p
  upgrade-interface` green; `cargo build` workspace green; a running network's
  genesis app-hash is **unchanged** (module absent from every registry vec).
- **Notes / risks:**
  - `MIN_UPGRADE_LEAD` must reference `CUTOVER_DELAY` (=3, `main.rs:132`), not a
    hardcoded divergent number, so `H` never lands inside an armed cutover window
    (risk R11).
  - `MAX_PROTOCOL_VERSION` is a per-node BUILD constant and must **never** enter
    this module — it varies per node and would fork consensus state. The honesty
    check lives node-side before emitting `SignalReady`; the module only records.
  - `ReadySignal.commitment` is folded into `root()`. Keep it `None` in phase 1
    (record presence only) so two honest nodes on the same `to_version` cannot
    diverge the upgrade-module root over per-node build metadata.
- **Depends on:** nothing. Independently PR-able.

---

### Phase 2 — Thread `protocol_version` (inert baseline, never hashed)

Add the read-only dispatch input to `BlockContext` and `Env`, stamp it as
`effective_version(height)` at the live seam, and provide the single shared host
accessor. One atomic PR (the field sweep must compile as a unit).

#### Task 2.1 — sdk `Env.protocol_version`
- **File to modify:** `crates/kernel/sdk/src/lib.rs` (`Env` struct l.140-149).
- **Change:** add `pub protocol_version: u32`, documented as: copied verbatim from
  `BlockContext` by the host drain; the ONLY version signal a module may branch on
  inside `execute()`/`query()`; NEVER folded into `root()` or op-encoding bytes. To
  bound churn across the ~50 `Env { .. }` literal sites (mostly test-mock `Ctx`
  impls across `crates/apps/*/tests` and `src`), add either `impl Default for Env`
  (requires `Origin: Default` — derive it with `Origin::System` as `#[default]`, or
  a manual impl) or an `Env::block(height, consensus_time, origin, me,
  protocol_version)` constructor, and sweep the mock sites mechanically.
- **Key type:**
  ```rust
  pub struct Env {
      pub height: u64, pub consensus_time: u64, pub origin: Origin, pub me: ModuleId,
      /// verbatim copy of BlockContext.protocol_version; the only version signal a
      /// module may branch on; never folded into root()/op bytes.
      pub protocol_version: u32,
  }
  ```

#### Task 2.2 — host `BlockContext.protocol_version` + drain stamping + accessor
- **File to modify:** `crates/kernel/host/src/lib.rs`.
- **File to modify:** `crates/kernel/host/Cargo.toml` — add `upgrade-interface = {
  workspace = true }` to `[dependencies]` (required because `Host::effective_version`
  below calls `upgrade_interface::effective_version` + decodes the `Status` reply).
  Host takes a TYPES-ONLY interface dependency here; its module-agnostic contract is
  preserved because `upgrade-interface` is an interface crate, not a module impl (same
  shape as governance depending on `valset-interface`).
- **Change:**
  1. Add `pub protocol_version: u32` to `BlockContext` (l.46-53); set it to
     `BASELINE_VERSION` (a new const `= 1`) in `Default` (l.55-65) so `submit()`
     stays byte-identical.
  2. In `drain` read `let protocol_version = ctx.protocol_version;` alongside
     height/consensus_time and stamp it into the per-dispatch `Env` literal so it
     is constant across the root op and all FIFO follow-ups.
  3. Populate `protocol_version` in the two query `Env` literals: the
     `ReadOnlyQueryCtx` nested ctx (copy from `self.env.protocol_version`) and the
     external `Host::query` ctx (hardcodes height 0 — stamp `BASELINE_VERSION`;
     external reads documented as baseline-format, risk R below).
  4. Add `pub async fn effective_version(&self, height: u64) -> u32` that reads the
     committed upgrade-module state (via existing `self.query(UPGRADE_MODULE_ID,
     encode_query(&UpgradeQuery::Status))` routing — outside any block =
     end-of-`H-1` committed state), applies the shared
     `upgrade_interface::effective_version`, and **falls back to
     `BASELINE_VERSION` when the upgrade module id is absent** (pre-retrofit nets)
     rather than erroring.
  - Confirm `global_root`/`app_hash` fold only `(id, root())` — `protocol_version`
    is structurally never in the preimage.
- **Key type:**
  ```rust
  pub struct BlockContext {
      pub height: u64, pub consensus_time: u64, pub origin: Origin,
      /// effective_version(height); read-only dispatch input, NEVER hashed.
      pub protocol_version: u32,
  }
  impl Default for BlockContext { /* ..., protocol_version: BASELINE_VERSION */ }
  impl Host { pub async fn effective_version(&self, height: u64) -> u32 { /* reads committed upgrade state; BASELINE if absent */ } }
  ```

#### Task 2.3 — node stamping seam
- **File to modify:** `crates/kernel/node/src/lib.rs` (the `BlockContext` literal at
  l.972-976).
- **Change:** before building `BlockContext`, `let protocol_version =
  self.host.effective_version(height).await;` and set it on the literal. This MUST
  be `effective_version(height)` (the pure derivation), NOT the raw stored
  `current_version` — otherwise block `H` dispatches under OLD logic while each
  changed module's `active_version` already selects NEW in `root()` (the
  H-hashes-new/dispatches-old off-by-one, spec l.330-340). This is the only
  per-block dispatch-version set; `OrderedNode::submit` is unchanged.
- **Tests:** new `crates/kernel/host/tests/protocol_version_threading.rs`:
  - drain copies `BlockContext.protocol_version` into `Env.protocol_version`
    identically on the root op and on every emitted follow-up (a probe module
    records `env().protocol_version` per dispatch).
  - **never-hashed invariance:** two blocks with different `protocol_version` but
    identical ops against a probe module whose `root()` ignores `Env` ⇒ `app_hash`
    and every module root byte-identical.
  - `BlockContext::default().protocol_version == BASELINE_VERSION`; legacy
    `submit()` path unchanged.
  - `Host::effective_version(height)` returns `to_version` at/after an armed
    pending `H`, `current_version` below `H`, and `BASELINE_VERSION` with no
    upgrade module registered (no panic).
- **Acceptance:** `crates/kernel/host/tests/protocol_version_threading.rs` green;
  full workspace `cargo test` green after the mechanical literal sweep.
- **Risks:** R3 (two stamping seams must agree — enforced by both calling
  `Host::effective_version` in Phases 2/6); R5 (never-hashed regression — the
  invariance test is the standing guard); R12 (~80 literal sites — a miss is a
  compile error, mitigated by the Default/constructor helper); external/query
  height-0 read paths stamp baseline (read-correctness wart, not consensus).
- **Depends on:** Phase 1 (accessor uses `upgrade-interface::effective_version` +
  the `Status` query decode).

---

### Phase 3 — Governance `ScheduleUpgrade` / `CancelUpgrade`

Add the two authorize-only `GovAction` variants and the upgrade-module follow-up
emit, mirroring how `AddValidator` emits a valset `Join`. One atomic PR (the
`Governance::new` signature change ripples to 7 call sites).

#### Task 3.1 — Interface variants
- **File to modify:** `crates/system/governance-interface/src/lib.rs` (`GovAction`
  l.19-26).
- **Change:** add `ScheduleUpgrade { name: String, activation_height: u64,
  to_version: u32 }` and `CancelUpgrade { name: String }`. Update the enum doc to
  note they authorize (schedule) but do NOT arm. Types-only; do **not** add an
  `upgrade-interface` dep here (`GovAction` rides inside `GovMsg` via serde_json, so
  no encode/decode fn changes).
- **Key type:**
  ```rust
  pub enum GovAction {
      AddValidator { key: Vec<u8> },
      RemoveValidator { key: Vec<u8> },
      Signal { text: String },
      ScheduleUpgrade { name: String, activation_height: u64, to_version: u32 }, // authorize only
      CancelUpgrade { name: String },
  }
  ```

#### Task 3.2 — Governance impl (ctor, encode/decode tags, door-check, emit)
- **File to modify:** `crates/system/governance/src/lib.rs`.
- **Change:**
  1. Struct + ctor: add `upgrade_id: ModuleId` (mirrors `valset_id`, l.59-79);
     `new(id, valset_id, upgrade_id)`.
  2. Imports: `use upgrade_interface::{UpgradeMsg, encode_msg as upgrade_encode_msg};`.
  3. `encode_state` (root preimage, l.135-169): add match arms with **NEW tag
     bytes 3 and 4** — never renumber 0/1/2. `ScheduleUpgrade` ⇒ `out.push(3)` +
     `push_bytes(name)` + `activation_height.to_le_bytes()` +
     `to_version.to_le_bytes()`; `CancelUpgrade` ⇒ `out.push(4)` +
     `push_bytes(name)`.
  4. `decode_state` action match (l.452-463): `3 => ScheduleUpgrade { name:
     take_string, activation_height: take_u64, to_version: take_u32 }`, `4 =>
     CancelUpgrade { name: take_string }`. Add a `take_u32` helper next to
     `take_u64` (l.403), using `split_first_chunk::<4>`.
  5. `handle_propose` door-check (l.215-223): reject an empty `name` for
     `ScheduleUpgrade`/`CancelUpgrade` (a proposal that can never execute is
     rejected at the door). Do **not** duplicate monotonicity/min-lead/at-most-one
     — those are the upgrade module's sole authority (risk R9).
  6. `handle_execute` follow-up match (l.307-322): add
     ```rust
     GovAction::ScheduleUpgrade { name, activation_height, to_version } => ctx.emit_msg(Msg {
         target: self.upgrade_id.clone(),
         payload: upgrade_encode_msg(&UpgradeMsg::Schedule {
             name: name.clone(), activation_height: *activation_height, to_version: *to_version }),
     }),
     GovAction::CancelUpgrade { name } => ctx.emit_msg(Msg {
         target: self.upgrade_id.clone(),
         payload: upgrade_encode_msg(&UpgradeMsg::Cancel { name: name.clone() }),
     }),
     ```
     The host stamps this follow-up `Origin::Module("governance")` automatically,
     which is exactly the origin the upgrade module's gate accepts. The tally is
     UNCHANGED (simple majority `members.len()/2+1`, l.296); readiness/`R=n` is
     NEVER read here (threshold separation, risk R below).
- **File to modify:** `crates/system/governance/Cargo.toml` — add `upgrade-interface
  = { workspace = true }` to `[dependencies]` (mirrors the `valset-interface` line,
  types-only). No `upgrade` dev-dep if the new test uses an in-test stub target
  module (recommended for isolation).

#### Task 3.3 — Ripple the ctor to all 7 call sites
- **Files to modify:**
  - `bin/node/src/main.rs` l.250, l.319, l.511 →
    `Governance::new("governance", "valset", "upgrade")`.
  - `crates/system/governance/tests/governance_gates_valset.rs` l.43, l.198, l.498,
    l.510 → the same 3-arg signature (these tests never exercise the new arms, so
    the id string alone suffices).
- **Tests:**
  - new `crates/system/governance/tests/governance_schedules_upgrade.rs` (through a
    REAL host with valset + governance + an in-test stub `upgrade` module):
    - `a_passing_schedule_upgrade_emits_the_upgrade_followup` — member proposes
      `ScheduleUpgrade`, members vote yes, Execute settles Passed AND the stub
      records `UpgradeMsg::Schedule` with `Origin::Module("governance")`.
    - `cancel_upgrade_emits_cancel_followup` — passing `CancelUpgrade` emits
      `UpgradeMsg::Cancel { name }`.
    - `outsider_cannot_propose_schedule_or_cancel` — non-member propose refused
      with a member error (reuses `require_member`).
    - `empty_name_rejected_at_propose` — door-check refuses empty name before tally.
    - `rejected_followup_fails_execute_atomically` — a stub returning `Err` on an
      invalid `Schedule` fails the Execute op, the proposal stays Open (staged
      pending discarded), no partial state (governance is not a second authority).
  - extend `governance_gates_valset.rs` snapshot test to include a `ScheduleUpgrade`
    and a `CancelUpgrade` proposal in state (round-trips tag 3/4 + install
    root-matches).
  - `root_preimage_stable_for_existing_variants` — a state of only tag-0/1/2
    proposals encodes byte-identically before and after this change (app-hash
    continuity guard, risk R8).
- **Acceptance:** `cargo test -p governance` green, including the byte-identical
  encode guard.
- **Risks:** R8 (fresh tags 3/4, never renumber 0/1/2); R9 (two-authority hazard —
  door-check limited to non-empty name; the module's `Err` fails Execute
  atomically); threshold separation (schedule path never reads `R=n`); constructor
  ripple (7 sites). Safe pre-registration: an actual `ScheduleUpgrade` before
  Phase 8 would emit to an absent `"upgrade"` ⇒ `UnknownModule` ⇒ Execute fails
  atomically (no fork), and operationally none is proposed yet.
- **Depends on:** Phase 1 (needs `UpgradeMsg`).

---

### Phase 4 — Manifest version fields + boot preflight (recovery + state-sync)

Additive fields with tolerant/schema-tagged decode, plus a pure fail-loud preflight
helper. Inert: on a net without the module, capture reads baseline defaults.

#### Task 4.1 — sdk shared types + preflight helper
- **File to modify:** `crates/kernel/sdk/src/lib.rs` (near `ModuleId`/`StateRoot`).
- **Change:** add a serializable MIRROR of the module's pending coords (NOT the
  authority) so neither manifest crate depends on the upgrade module, plus a pure
  dependency-free preflight.
  ```rust
  #[derive(Clone, Debug, PartialEq, Eq)]
  pub struct UpgradeCoords { pub name: String, pub activation_height: u64, pub to_version: u32 }

  #[derive(Debug)]
  pub struct UnsupportedVersion { pub required_min: u32, pub max_supported: u32 }
  // Display: "this boundary needs protocol v{required_min}; binary supports up to
  //           v{max_supported} — install the newer node binary"

  pub fn check_required_version(required_min: u32, max_supported: u32) -> Result<(), UnsupportedVersion> {
      if max_supported < required_min { Err(UnsupportedVersion { required_min, max_supported }) } else { Ok(()) }
  }
  ```

#### Task 4.2 — recovery `Manifest`
- **File to modify:** `crates/kernel/recovery/src/lib.rs`.
- **Change:** Manifest struct (l.329-365) gains `current_version: u32`,
  `pending_upgrade: Option<UpgradeCoords>`, `required_min_version: u32` (mirroring
  the existing optional `pending_cutover_view`). `encode` (l.367-397): append
  `put_u32(current_version)`, a 0/1 presence-tagged `pending_upgrade`, then
  `put_u32(required_min_version)` — and **bump a manifest schema tag** (or lead the
  manifest with a version byte) so a NEW binary reading an OLD on-disk checkpoint
  maps a missing tail to defaults (`current_version = BASELINE`, `pending_upgrade =
  None`, `required_min_version = BASELINE`) rather than tripping the `c.done()?`
  no-trailing-bytes check (l.428). `decode` (l.399-441): read the three fields
  symmetrically with schema-tag/tolerant-tail handling. `capture` (l.462-505): add
  params `current_version: u32`, `pending_upgrade: Option<UpgradeCoords>`; compute
  `required_min_version = to_version` when `height >= pending.activation_height`
  (pending present), else `current_version`. Add `pub fn preflight(&self,
  max_supported: u32) -> Result<(), UnsupportedVersion>` delegating to
  `sdk::check_required_version`, plus a `required_min_version()` accessor. Recovery
  itself performs NO re-arm and NO version derivation — the `Cutover` replay handler
  (l.840-852) and `apply_block` (l.996-1017) are UNCHANGED (activation is
  execute-time and lands via the module's own committed state during replay).
- **Key type:**
  ```rust
  pub fn capture(host: &Host, height: Option<u64>, epoch: u64, view_base: u64,
                 participants: Vec<Vec<u8>>, pending_cutover_view: Option<u64>,
                 current_version: u32, pending_upgrade: Option<UpgradeCoords>,
                 oplog_pos: u64, next_seq: u64) -> Result<Self, Error> { /* required_min_version derived */ }
  ```

#### Task 4.3 — state-sync `Manifest` + `BoundaryCoords`
- **File to modify:** `crates/kernel/statesync/src/lib.rs`.
- **Change:** Manifest struct (l.132-150) gains the same three fields, documented as
  UNAUTHENTICATED serving hints under the untrusted-server model (l.21-29) — a lying
  value can at worst mis-preflight a joiner (refuse-to-boot DoS, or boot-then-halt
  at the app-hash), never fork. `BoundaryCoords` (l.405-410) gains `current_version:
  u32` + `pending_upgrade: Option<UpgradeCoords>` (server stamps from live state
  like epoch/view_base). `encode_response` Manifest arm (~l.241-263): append the
  three fields (u32 + presence-tagged `UpgradeCoords` + u32) — a wire-format bump;
  mixed-binary interop is acceptable only because these fields arrive via the Phase
  8 stop-the-world retrofit, not a rolling upgrade. `decode_response` Manifest arm
  (~l.286-334): read them symmetrically before `expect_empty` with the existing
  forged-count bounds guards. Thread the fields through `try_handle`,
  `ensure_capture`, and the two other Manifest construction sites. Add
  `Manifest::preflight(&self, max_supported)` mirroring recovery's.
- **Tests:**
  - recovery: extend `manifest_roundtrip` (Some + None `pending_upgrade`,
    `current_version`, `required_min_version`); new
    `manifest_decode_tolerates_old_format` (an old-schema buffer decodes to baseline
    defaults); `required_min_version_fencepost` (`== current_version` below
    activation, `== to_version` at/after); `preflight_rejects_under_versioned_binary`
    / `accepts_sufficient`.
  - statesync: extend `response_frames_round_trip` (Some + None);
    `decode_response_rejects_truncated_version_tail` (clean failure, no panic);
    preflight pass/fail.
  - sdk: `check_required_version` boundary test (`max == required_min` passes,
    `max < required_min` fails).
- **Acceptance:** `cargo test -p recovery`, `cargo test -p statesync`, `cargo test
  -p sdk` green; the tolerant-decode test proves an on-disk checkpoint written by the
  prior binary still boots.
- **Risks:** R6 (format bump — mandatory schema tag / tolerant tail on the recovery
  side; statesync mixed-binary acceptable only under the Phase 8 retrofit);
  `required_min_version` is boundary-scoped (a joiner that syncs below `H` then
  advances live past `H` is not protected by preflight — intended, app-hash is
  authority, must be documented); capture must fail the checkpoint loudly on a
  failed upgrade-state read rather than silently defaulting.
- **Depends on:** Phase 1 for real values (defaults to baseline/None when absent).
  The sdk helper + tolerant decode are independently PR-able ahead of it.

---

### Phase 5 — Forge dual-path field + branch points (inert; default = CURRENT behavior)

Wire the branch selector into forge without changing behavior. **Critical
grounding:** `dev`'s forge `root()` (l.790-791) ALREADY uses the multi-repo
`compose_root`, so the inert default `active_version` MUST reproduce the current
`compose_root` byte-for-byte (see risk R7 — do NOT default to a legacy single-head
that would move the current root).

#### Task 5.1 — Forge `active_version` field + setter + branch points
- **File to modify:** `crates/apps/forge/src/lib.rs`.
- **Change:**
  1. Add `active_version: u32` to `struct Forge` (l.408-425), documented as NEVER
     part of the `root()`/`snapshot()` preimage; defaulting to the genesis baseline
     in `init`/`with_blobs` (l.432-501), set deterministically at `H` by the
     activation hook, restored per replayed/synced height.
  2. Add a deterministic setter `fn set_active_version(&mut self, v: u32)` for the
     activation hook (Phase 6) to drive.
  3. Branch `root()` (l.790-796), `snapshot()` (l.529+), and `install()` (l.570+)
     on `self.active_version` (the branch points; the actual OLD v1 bodies are the
     Phase-9 forge-dual-path work). The inert default branch = the current
     `compose_root` path unchanged.
  4. In `execute()` (l.811-834) and `query()` (l.841+) branch op/wire semantics on
     `ctx.env().protocol_version` (e.g. `norm_repo` repo-field collapse below `H`);
     inert default = current behavior.
- **Key type:**
  ```rust
  pub struct Forge {
      id: ModuleId, base: PathBuf, blobs: files::BlobHandle, repos: BTreeMap<String, RepoState>,
      /// cached branch selector, set at H by the activation hook; NEVER in root()/snapshot() preimage.
      active_version: u32,
  }
  impl Module for Forge {
      fn root(&self) -> StateRoot { /* branch on self.active_version; default = current compose_root */ }
  }
  ```
- **Tests:** new `crates/apps/forge/tests/active_version_branch.rs` — `root()` /
  `snapshot()` / `install()` branch on `active_version`; flipping `active_version`
  recomputes `root()` from in-memory `RepoState.head` values with zero odb/blob IO;
  a snapshot round-trips within a fixed `active_version`. Existing forge tests
  unchanged (the default branch is byte-identical to today).
- **Acceptance:** `cargo test -p forge` green (existing + new); the default
  `active_version` produces the pre-change `root()`/snapshot bytes exactly.
- **Risks:** R7 (default-branch decision — the Phase-5 inert default MUST equal
  whatever baseline Phase 8 commits to; the cleanest inert choice is default =
  current `compose_root`, with the OLD single-head becoming a *lower* version that
  only a future scheduled downgrade-of-format could select — but since version is
  monotonic, prefer treating current `compose_root` as the baseline and
  demonstrating in Phase 9 with a fresh higher `to_version`); active_version
  staleness (`root()` reads the field but block `H` may not dispatch forge — the
  setter must be driven by the activation hook independent of dispatch, Phase 6).
- **Depends on:** Phase 2 (for `ctx.env().protocol_version`). Otherwise independent.

---

### Phase 6 — Activation boundary in `ValsetOrchestrator` + driver wiring

Extend the finalized cutover the orchestrator already crosses to also carry the
boundary protocol version and evaluate the `R=n` arm/abort verdict exactly once
against the frozen readiness set + boundary valset. One atomic PR (breaking
`respawn_if_due` signature + every orchestrator test).

#### Task 6.1 — Orchestrator types + pure verdict + extended plan/signature
- **File to modify:** `crates/kernel/consensus/src/valset_orchestrator.rs`.
- **Change:**
  1. Add version-carrying types: `BoundaryUpgrade<Member> { current_version: u32,
     pending: Option<PendingUpgrade<Member>> }`; `PendingUpgrade<Member> { name:
     String, activation_height: u64, to_version: u32, ready: BTreeSet<Member> }`;
     `UpgradeVerdict { None, Armed { name, to_version }, Abort { name } }`.
  2. Extend `RespawnPlan<Member>` (l.67) with two NON-hashed fields `boundary_version:
     u32`, `upgrade_verdict: UpgradeVerdict` + accessors `boundary_version()` /
     `upgrade_verdict()`.
  3. Add a pure private `fn arm_verdict(boundary_app_height: u64, up:
     &BoundaryUpgrade<Member>, boundary_valset: &BTreeSet<Member>) -> (u32,
     UpgradeVerdict)`: `pending.activation_height <= boundary_app_height &&
     !boundary_valset.is_empty() && boundary_valset ⊆ ready` ⇒ `(to_version,
     Armed)`; reached but subset fails ⇒ `(current_version, Abort)`; else ⇒
     `(current_version, None)`. THIS is the single source of truth the module's
     `Advance` derivation must mirror (risk R4).
  4. Change `respawn_if_due` (l.241) signature to also take `boundary_upgrade:
     BoundaryUpgrade<Member>`; after taking the single pending cutover exactly once,
     compute `cutover_app_height`, build the boundary valset, call `arm_verdict`
     ONCE, and stamp `boundary_version` + `upgrade_verdict` into the plan.
  5. Add `observe_upgrade(&mut self, finalized_view, activation_app_height) ->
     ObservationOutcome`: arms the SINGLE pending slot at `cutover_view =
     activation_app_height - epoch_base` (checked_sub, fail-stop) ONLY when the slot
     is empty and the view is strictly future; when a membership cutover already
     holds the slot it returns `Pending` (the version flip rides that boundary via
     the boundary read — never a competing arm).
  6. Extend `resume` (l.146) to re-arm a version-scheduled cutover from recovered
     coordinates the same way `pending_cutover_view` re-arms a membership cutover
     (the single slot is shared).
- **File to modify:** `crates/kernel/consensus/src/lib.rs` — extend the `pub use
  valset_orchestrator::{...}` re-export (l.72-74) to also export `BoundaryUpgrade`,
  `PendingUpgrade`, `UpgradeVerdict`.
- **Key types:**
  ```rust
  pub struct RespawnPlan<Member> {
      epoch: u64, epoch_base: u64, cutover_view: u64, cutover_app_height: u64,
      valset: EpochMembership<Member>,
      boundary_version: u32,          // NEW: dispatch-only, NEVER hashed
      upgrade_verdict: UpgradeVerdict, // NEW: dispatch-only, NEVER hashed
  }
  pub enum UpgradeVerdict { None, Armed { name: String, to_version: u32 }, Abort { name: String } }
  fn arm_verdict(boundary_app_height: u64, up: &BoundaryUpgrade<Member>, boundary_valset: &BTreeSet<Member>) -> (u32, UpgradeVerdict);
  pub fn respawn_if_due(&mut self, finalized_view: u64, boundary_members: impl IntoIterator<Item = Member>, boundary_upgrade: BoundaryUpgrade<Member>) -> Option<RespawnPlan<Member>>;
  ```

#### Task 6.2 — Driver wiring (sole orchestrator call site)
- **File to modify:** `bin/node/src/main.rs`.
- **Change:**
  1. Add `const MIN_UPGRADE_LEAD: u64` `>= CUTOVER_DELAY` (=3, l.132), documented as
     the schedule-time floor the module enforces so `H` is strictly future.
  2. Add `read_upgrade_state(host) -> BoundaryUpgrade<ed25519::PublicKey>` mirroring
     `read_valset_members` (l.198): decode `current_version`, pending `Upgrade`
     coords, and the readiness map into decoded pubkeys.
  3. In the orchestration step (l.2578-2597): after `observe_members`, read upgrade
     state and, when a pending upgrade exists, call
     `orchestrator.observe_upgrade(engine_view, activation_view)`; on `Scheduled`,
     call `node.set_view_ceiling(cutover_view)` exactly like the membership branch.
  4. Change the `respawn_if_due(engine_view, observed)` call (l.2597) to pass the
     freshly-read `BoundaryUpgrade`.
  5. On the returned plan: propagate `plan.boundary_version()` into
     `Forge::set_active_version` (the active_version realization). The upgrade
     module's OWN stored-state reconciliation — `current_version = to_version` +
     clear on Armed, clear-only on Abort — is driven by the single System-origin
     `Advance` injection of Task 6.3 (below), which lands at the SAME finalized view
     so `pending` + `readiness` clear on BOTH outcomes (not just Abort). Do NOT branch
     a separate abort-only follow-up here — the one `Advance` handler owns both.
  6. Resume path (~l.2148): thread the recovered pending-upgrade coordinates into
     `ValsetOrchestrator::resume` and re-arm the ceiling like `pending_boot`.

#### Task 6.3 — System-origin `Advance` boundary injection (stored-state reconciliation)
- **File to modify:** `bin/node/src/main.rs` (the same orchestration step as Task 6.2).
- **Change:** At the finalized teardown-respawn boundary that crosses a pending
  upgrade's `activation_height`, the driver injects EXACTLY ONE `Origin::System`
  `UpgradeMsg::Advance` op through the same `host.submit_at` drain point the boundary
  already uses — an execute-time, in-block transition, never a respawn side-effect,
  emitted unconditionally whenever the pending's `H` is reached (arm OR abort). The
  module's `Advance` handler (Task 1.2) then deterministically re-evaluates the
  IDENTICAL arm predicate as `arm_verdict` / `effective_version` against COMMITTED
  state + the frozen boundary set and reconciles its own stored state:
  - **ARM** (`H` reached, `R == n`): `current_version = to_version` **and** clear
    `pending` + `readiness`.
  - **ABORT** (`H` reached, `R < n`): clear `pending` + `readiness`, leaving
    `current_version` unchanged (identical effect to the abort follow-up it replaces).
  - Idempotent no-op when there is no pending or it is already reconciled.
  Because `Advance` is System-origin it passes the module's Schedule/Cancel/Advance
  origin gate, and because it lands in-block it reconstructs byte-for-byte on live,
  recovery-replay, and state-sync-join nodes (the spec's execute-time invariant).
  **This is what frees the at-most-one-pending slot after a successful activation:**
  without the ARM-path clear, `pending` never clears, so the at-most-one-pending rule
  would permanently reject every SECOND `ScheduleUpgrade`. Since the boundary sets
  `Forge.active_version` (dispatch) while `Advance` reconciles the upgrade module's
  own committed `current_version`, both land at the one finalized view H and every
  node agrees.
- **Tests** (driver-level, alongside `crates/kernel/consensus/tests/valset_orchestrator.rs`):
  - `advance_arm_reconciles_and_frees_slot` — the injected `Advance` at an armed
    boundary sets `current_version == to_version`, clears `pending` + `readiness`
    (`Status.pending == None`), and a fresh `ScheduleUpgrade` is then ACCEPTED.
  - `advance_abort_clears_without_flip` — at an aborted boundary clears `pending` +
    `readiness` with `current_version` unchanged, and a fresh `ScheduleUpgrade` is
    likewise accepted.
  - `advance_is_idempotent` — a second `Advance` (or an `Advance` with no pending) is
    a no-op leaving committed state + root byte-identical.
- **File to modify:** `crates/kernel/consensus/tests/valset_orchestrator.rs` — add
  the new tests below; update existing `respawn_if_due` call sites to pass a
  no-pending `BoundaryUpgrade` and assert `boundary_version == current_version` /
  verdict `None`.
- **Tests** (`crates/kernel/consensus/tests/valset_orchestrator.rs`):
  - `version_flip_arms_cutover_at_activation_height` — `observe_upgrade` on an empty
    slot arms at `cutover_view = H - epoch_base`; a second `observe_upgrade` returns
    `Pending`.
  - `boundary_read_flips_when_R_equals_n` — ready ⊇ boundary_members and
    `activation_height <= cutover_app_height` ⇒ plan `boundary_version == to_version`,
    verdict `Armed`.
  - `straggler_aborts_upgrade_cleanly` — one member missing ⇒ `boundary_version`
    stays current, verdict `Abort { name }`.
  - `non_member_ready_signals_are_dead_weight` — extra non-member key + a real member
    missing ⇒ `Abort`; all members ready + extra dead keys ⇒ `Armed`.
  - `coincident_membership_and_version_share_one_respawn` — one plan carries the new
    valset AND `boundary_version == to_version`.
  - `version_cutover_absorbs_membership_change_inside_window` — version cutover armed
    first; membership change inside the window returns `Pending`; `respawn_if_due`
    reads the new members AND flips in one plan.
  - `effective_version_pure_below_H` — a membership boundary firing at app-height `<
    activation_height` yields verdict `None`, version does not flip early.
  - `abort_verdict_evaluated_exactly_once` — the single slot is consumed once; a
    second call returns `None`.
  - `resume_rearms_pending_upgrade` — `resume` with recovered coords re-arms the same
    deterministic `H` and flips like an uninterrupted peer.
  - Update `respawn_waits_until_cutover_view`,
    `boundary_read_absorbs_a_second_change_inside_the_window`,
    `app_height_continues_across_respawn`, `resume_rearms_a_pending_cutover` to pass
    a no-pending `BoundaryUpgrade` and assert the pure-membership path is byte-unchanged.
- **Acceptance:** `cargo test -p consensus` green (new + updated); `cargo build -p
  node` green (driver compiles against the new signature); the injected `Advance`
  reconciles stored state on BOTH verdicts so `pending` + `readiness` clear after a
  successful activation and the at-most-one-pending slot is free for a second
  `ScheduleUpgrade`.
- **Risks:** R4 (`arm_verdict` must be byte-identical to the module's `Advance`
  derivation — one shared predicate); `boundary_version`/`upgrade_verdict` must
  never enter a hashed preimage (doc-comment + the Phase-2 invariance test); the
  System-origin `Advance` must be injected unconditionally whenever a pending's `H`
  is reached (arm OR abort) so `pending`/`readiness` clear at one finalized view
  either way and the pending slot frees on activation (Task 6.3);
  app-height→engine-view conversion must be checked (fail-stop) and re-derived after
  each epoch rebase; breaking `respawn_if_due` signature (pass no-pending everywhere).
- **Runbook note (admission during an open window):** a validator admitted between
  `ScheduleUpgrade` and `H` legitimately enters the boundary readiness denominator
  (`R = n`), so it must be provisioned new-binary-first or the upgrade cleanly aborts.
  That admission-gating-during-an-open-window is owned by the `/upgrade` skill runbook
  (its Step 2 already covers it), not this plan — so the spec Edge case "New validator
  admitted between `ScheduleUpgrade` and `H`" is complete on this point.
- **Depends on:** Phases 1, 2, 4, 5. Testable inert (unit tests use synthetic
  `BoundaryUpgrade`; `read_upgrade_state` returns baseline when the module is absent).

---

### Phase 7 — Node runtime wiring: `ReadinessSignaller`, `upgrade-status` CLI, markers, capture reads

The `ducktape-node` layer that self-emits `SignalReady`, exposes an
`upgrade-status` subcommand, prints greppable transition markers, and passes the
new version fields into every `Manifest::capture`. All degrade gracefully when the
module is absent (inert before Phase 8).

#### Task 7.1 — `MAX_PROTOCOL_VERSION` + `ReadinessSignaller`
- **File to modify:** `bin/node/src/main.rs`.
- **Change:**
  1. Add `const MAX_PROTOCOL_VERSION: u32 = 1;` next to `CONSENSUS_SCHEME` (l.71) —
     the highest protocol version this build's dual-path modules implement (bumped
     to 2 in Phase 9). `SignalReady` is truthful iff `MAX_PROTOCOL_VERSION >=
     to_version`.
  2. Add a node-local `ReadinessSignaller` (deliberately NOT a `reactor::Worker` —
     readiness must survive restart/late-join, so it polls COMMITTED upgrade state
     each pump tick, idempotently) constructed alongside the `workers` vec (~l.2244),
     seeded with `MAX_PROTOCOL_VERSION` and `signer.public_key()`. Between drains
     (the cutover-nop pusher seam ~l.2685) call `signaller.maybe_signal(node.host())`
     and, on `Some(msg)`, self-submit via `node.submit(&signer, next_seq, msg)`,
     printing `[node LABEL] signaled ready name=… to_version=N`. Gate to VALIDATORS
     only and only when self is a current boundary member.
  3. After each drain batch, query the module's effective_version and print one-shot
     transition markers (`upgrade armed name=… to_version=N height=H`, `upgrade
     activated version=N at height=H`, `upgrade cleared name=…`), modeled on the
     `converged` latch (~l.2740).
- **Key type:**
  ```rust
  struct ReadinessSignaller { max_version: u32, me: Vec<u8>, signaled: Option<(String, u32)> }
  impl ReadinessSignaller {
      async fn maybe_signal(&mut self, host: &Host) -> Option<Msg> {
          let reply = host.query("upgrade", &upgrade_interface::encode_query(&UpgradeQuery::Status)).await.ok()?;
          let UpgradeReply::Status(st) = upgrade_interface::decode_reply(&reply).ok()? ;
          let p = st.pending?;                          // no pending -> nothing to do
          if p.to_version > self.max_version { return None }   // binary too old
          if st.ready.contains(&self.me) { return None }       // module already has our signal
          if self.signaled.as_ref() == Some(&(p.name.clone(), p.to_version)) { return None } // in flight
          self.signaled = Some((p.name.clone(), p.to_version));
          Some(Msg { target: "upgrade".into(),
                     payload: upgrade_interface::encode_msg(&UpgradeMsg::SignalReady {
                         name: p.name, to_version: p.to_version, commitment: None }) })
      }
  }
  ```

#### Task 7.2 — `upgrade-status` subcommand + capture reads
- **File to modify:** `bin/node/src/main.rs`.
- **Change:** add `Some("upgrade-status") => return cmd_upgrade_status(&args[1..]),`
  to the subcommand match (l.744-751), plus `fn cmd_upgrade_status` driving the local
  rpc `Query` against `"upgrade"` (and `"valset"` for the `R=n` denominator) and
  printing `current_version` / pending / readiness count / effective_version. Pass
  `current_version` + `pending_upgrade` into every `Manifest::capture` call site
  (genesis l.1789, promotion l.1725, and the periodic checkpoints) from a
  `host.query("upgrade", …)` read (baseline default / None when absent). Add
  `"upgrade"` to `MODULE_IDS` is deferred to Phase 8 (that is the registration).
- **Key type:**
  ```rust
  fn cmd_upgrade_status(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
      let (_, flags) = parse_flags(args)?;
      let resolved = config::resolve(&PathBuf::from(flags.get("config").map(String::as_str).unwrap_or("node.toml")))?;
      let addr = resolved.rpc_listen.ok_or("upgrade-status drives the node rpc — set rpc_listen")?;
      let raw = rpc_query(&addr, "upgrade", &upgrade_interface::encode_query(&UpgradeQuery::Status))?;
      let members = read_members(&addr)?;
      // print current_version, pending {name,H,to_version}, readiness N of members.len(),
      // effective_version(current_height), armed (R==n).
      Ok(())
  }
  ```

#### Task 7.3 — Invoke the boot preflight on the real resume + join paths
- **File to modify:** `bin/node/src/main.rs`.
- **Change:** At node boot, BEFORE any replay/serve begins, call the manifest
  preflight (Phase 4) against the node build constant `MAX_PROTOCOL_VERSION` (Task
  7.1) on BOTH boot paths that Phase 4 only equipped with the method:
  1. **recovery-resume** — after loading the on-disk recovery `Manifest`, call
     `manifest.preflight(MAX_PROTOCOL_VERSION)` (i.e.
     `check_required_version(manifest.required_min_version(), MAX_PROTOCOL_VERSION)`)
     before handing off to recovery replay.
  2. **state-sync-join** — after receiving the served state-sync `Manifest`, call the
     same preflight before install/replay.
  On `Err(UnsupportedVersion)` abort boot FAIL-LOUD with the Display message ("height
  N requires binary vX" / "this boundary needs protocol vX; binary supports up to vY
  — install the newer node binary") and a non-zero exit — the same fail-loud posture
  as the pre-recovery boot halt — instead of falling through to an opaque post-replay
  `AppHashMismatch` (`recovery/src/lib.rs:971-975`). On `Ok(())` boot proceeds.
- **Tests:** a recovery `Manifest` and a state-sync `Manifest` whose
  `required_min_version > MAX_PROTOCOL_VERSION` each abort boot early with the version
  message and NO replay/serve attempted; a sufficient binary boots. Real early-refusal
  is asserted in Phase 9. Inert before Phase 8 (baseline `required_min_version` always
  passes).

- **Tests:** compiles + runs inert on a net without the module (signaller queries
  the absent module → `None`, no panic; CLI reports baseline). Real behavior is
  asserted in Phase 9's e2e.
- **Acceptance:** `cargo build -p node` green; `ducktape-node upgrade-status` runs
  against a live baseline node and reports baseline without panicking.
- **Risks:** R10 (`SignalReady` spam — local `(name,to_version)` dedupe + module
  idempotence; validators-and-current-member only; rely on the ordered lane's
  authenticated `Origin::External(pubkey)`, never a claimed origin field);
  `Manifest::capture` grows args, rippling to 4+ sites — a miss mis-stamps a
  checkpoint (annoying, not a fork; a dropped resume coord would rejoin the wrong
  `H`).
- **Depends on:** Phases 1, 3, 4, 6. Must degrade gracefully when the module id is
  absent.

---

### Phase 8 — ⚑ COORDINATED GENESIS BUMP (register the module — the stop-the-world flip)

The first landing that moves the genesis app-hash. Register the module in ALL
parallel module vecs in LOCKSTEP. This is a coordinated restart on `dev` (state
predating it is incompatible — fresh genesis); the separate live Ducktape-2 net
stays on its old binary, untouched.

#### Task 8.1 — Register `upgrade` in every module vec + bump `MODULE_IDS`
- **File to modify:** `bin/node/src/main.rs`.
- **Change (ONE atomic PR, all vecs lockstep):**
  1. `genesis_host` (the `Host::genesis(vec![...])` at l.242-282): add `Box::new(Upgrade::new("upgrade", "valset"))` alongside the `Governance` registration (l.250). Add `use upgrade::Upgrade;`.
  2. `restore_host` (l.290+): add `let mut upgrade = Upgrade::new("upgrade", "valset"); upgrade.install(snapshot_of("upgrade")?, …)?;` and add it to the vec (l.395-414) so a restored node reconstructs the identical registry / module count.
  3. `sync_all_modules` (l.493-604): add a `snapshot_of("upgrade")` install and add it to the compose vec (l.585-604).
  4. Bump `MODULE_IDS` from `[&str; 17]` to `[&str; 18]` (l.135) adding `"upgrade"` so Status RPC + http status report its root.
  5. Decide the forge baseline (risk R7) and optionally bump the genesis fingerprint at `config.rs:210-222` so old-binary nodes DISCONNECT rather than app-hash-fork (adding the module changes the app-hash but NOT the `sha256(scheme ‖ validators)` namespace, so without a fingerprint bump an old node would still handshake and then fork — a clean partition is strictly better).
- **File to modify:** `bin/node/Cargo.toml` — add `upgrade = { workspace = true }` to
  `[dependencies]`.
- **File to modify (optional):** `bin/node/tests/cluster_e2e.rs` — a comment noting
  the genesis/converged runtime hashes shifted intentionally (module count 17→18);
  no code change required (hashes are computed at runtime).
- **Tests:** a fresh `dev` cluster converges with 18 modules; the existing
  `cluster_e2e` kill+respawn parity and `run_sync_only` parity legs now transitively
  cover the newly-embedded module (the guard that all three vecs agree).
- **Acceptance:** `cargo test -p node --test cluster_e2e` green on a fresh cluster;
  restart parity and sync-only parity hold across the new module; genesis app-hash
  recomputes intentionally.
- **Risks:** R1 (genesis-bump chicken-and-egg — a module id cannot be introduced by
  a height-gated upgrade; treat this as an explicit coordinated genesis bump); R2
  (three-vec + `MODULE_IDS` lockstep — a partial change forks restart/sync; the
  restart + sync-only legs are the guard); the genesis-fingerprint decision (bump →
  clean partition vs no-bump → old nodes handshake-then-fork).
- **Depends on:** Phases 1-7 landed inert. **First landing that changes the genesis
  app-hash.**

---

### Phase 9 — Forge v2 demonstrator + `MAX_PROTOCOL_VERSION=2` + end-to-end test

Land a real dual-path target so a `root()` flip is observable, and prove the whole
flow end to end.

#### Task 9.1 — Forge v2 branch + `MAX_PROTOCOL_VERSION=2`
- **Files to modify:** `crates/apps/forge/src/lib.rs` (fill the OLD-vs-NEW bodies
  behind the Phase-5 branch points so a scheduled `to_version=2` actually changes
  `root()`/snapshot/op-routing); `bin/node/src/main.rs` bump `MAX_PROTOCOL_VERSION`
  to `2`.
- **Change:** wire the forge v2 branch as the first schedulable target (per the
  spec's worked example: below `H` the OLD preimage/format + `norm_repo`
  default-collapse; at/after `H` the sorted `compose_root` + multi-repo routing).
  The exact old/new bodies depend on the Phase-7/R7 baseline decision.
- **Acceptance:** `cargo test -p forge` green; a scheduled `to_version=2` produces a
  different `root()` at/after `H` and the byte-identical baseline below `H`.

#### Task 9.2 — End-to-end test
- **File to create:** `bin/node/tests/upgrade_e2e.rs`.
- **Tests** (reusing `common::Cluster` from `bin/node/tests/common/mod.rs` —
  `wait_marker`/`spawn_joiner`/`run_sync_only` — plus the cutover-filler pattern from
  `bin/node/tests/cluster_e2e.rs`):
  - `cluster_upgrade` (headline) — 3 validators + 1 future joiner: schedule
    `ScheduleUpgrade { name, activation_height: H, to_version: 2 }` via governance →
    every node's `ReadinessSignaller` auto-emits (`wait_marker "signaled ready
    name="`) → arm at `R=n` (`wait_marker "upgrade armed … to_version=2"`) → push
    fillers until `height >= H` → `wait_marker "upgrade activated version=2 at
    height=H"` on every validator → assert cross-node app-hashes AGREE at/after `H`
    (no fork) and differ from below `H` (forge root recomputed) → assert the
    upgrade-module `Status.pending` is now `None` (the at-most-one-pending slot was
    cleared by the `Advance` reconciliation of Task 6.3), and a fresh
    `ScheduleUpgrade { to_version: 3 }` is ACCEPTED — proving a SECOND upgrade can be
    scheduled after a successful activation.
  - `upgrade_restart_across_H` — kill+respawn a validator whose last committed
    height `>= H`; assert `recovered app_hash=` equals the live peers (recovery
    replayed through `H` under the v2 branch).
  - `upgrade_sync_only_across_H` — a fresh joiner state-syncs a boundary past `H`
    and composes the identical app-hash (`synced app_hash=` == boundary).
  - `upgrade_aborts_on_straggler` — a test-only `readiness_suppressed = true` config
    knob makes one node's signaller a no-op (simulating a straggler); assert `R=n` is
    never met, the clean abort fires at `H` (`upgrade cleared name=` on every node),
    and the network keeps finalizing on OLD logic (a post-`H` chat op still applies).
  - `upgrade_boot_preflight_refuses_under_versioned` — a restart/joiner running a
    binary with `MAX_PROTOCOL_VERSION=1` against a boundary whose
    `required_min_version=2` (captured at/after `H`) is REFUSED at boot EARLY with the
    "height N requires binary vX" message on BOTH the recovery-resume and
    state-sync-join paths, before any replay/serve — never falling through to a
    post-replay `AppHashMismatch` (exercises Task 7.3).
  - `upgrade_mixed_binary_no_partition` — a cluster mixing OLD-behavior
    (`MAX_PROTOCOL_VERSION=1`, no forge-v2 branch) and NEW (`MAX_PROTOCOL_VERSION=2`)
    dual-path binaries — both post-Phase-8, so both share the same genesis namespace
    `sha256(scheme ‖ validators)` — keeps handshaking and finalizing in lockstep for
    every height BELOW `H`; the mesh does NOT partition across the rolling window.
    This asserts the spec's structurally-load-bearing namespace invariant: version
    gating rides the app/consensus payload, never the p2p handshake.
  - `upgrade_status_cli` — after scheduling, run `ducktape-node upgrade-status`
    against a live node and assert stdout reports pending, readiness count, and
    effective_version.
- **Acceptance:** `cargo test -p node --test upgrade_e2e` green (all legs above),
  proving schedule → roll → arm `R=n` → flip at `H` with cross-node app-hash
  continuity, plus restart + sync-only parity across `H`, the clean abort, the
  early boot-preflight refusal of an under-versioned binary, the mixed-binary
  no-partition namespace check below `H`, and that `pending` clears after activation
  so a SECOND `ScheduleUpgrade` is accepted.
- **Risks:** R13 (a real `root()` flip is only observable once forge v2 +
  `MAX_PROTOCOL_VERSION=2` exist — hard sequencing behind Phase 8); the abort-path
  test's `readiness_suppressed` knob must be strictly test-gated so a production node
  can never withhold its own upgrade.
- **Depends on:** Phase 8.

---

## Independently PR-able vs must-co-land

- **Independent (parallelizable after their deps):** Phase 1; Phase 4 (the sdk
  helper + tolerant decode ahead of real values); Phase 5 **only after Phase 2** — it
  branches on `ctx.env().protocol_version`, so it is NOT independent of Phase 2.
- **Must co-land internally (single atomic PR each):** Phase 2 (sdk `Env` + host
  `BlockContext` + ~80 literal sites + node seam); Phase 3 (interface + governance +
  all 7 `Governance::new` sites); Phase 6 (orchestrator + re-exports + driver +
  every orchestrator test — breaking signature); **Phase 8 (all three module vecs +
  `MODULE_IDS` — a partial change forks restart/sync).**
- **Hard sequence:** 2→(5,6); 1→(2,3,4,6,7); 5→6; all→8→9.

## Genesis-bootstrap note

The `upgrade` module is a genesis constant: its mere presence in the host registry
IS its `global_root` contribution, and `protocol_version` is deliberately never
hashed — so a new module id cannot be introduced by a height-gated upgrade (there is
no prior in-hash version state to gate its own introduction on). Therefore Phase 8
is an explicit **coordinated genesis bump** on `dev`: land Phases 1-7 fully inert
first (module absent from every registry vec, so the running genesis app-hash does
not move), then flip all three module vecs + `MODULE_IDS` in one atomic PR. Adding
the module changes the app-hash but NOT the namespace `sha256(scheme ‖ sorted
validators)`, so without a genesis-fingerprint bump at `config.rs:210-222` an old
binary would still handshake and then fork — worse than a clean partition; prefer
bumping the fingerprint so old descriptors loudly fail to connect. The no-downtime
guarantee holds only for the SECOND and later upgrades, once the module is present
everywhere (the forge v2 demonstrator in Phase 9 is the first such no-downtime
upgrade). The separate live Ducktape-2 network stays on its old binary and is
untouched by this work.

## Risks and mitigations

| # | Risk | Phase | Mitigation |
|---|------|-------|-----------|
| R1 | Genesis-bump chicken-and-egg — a module id cannot be introduced by a height-gated upgrade (presence = `global_root` contribution; `protocol_version` never hashed). | 8 | Treat Phase 8 as an explicit coordinated `dev` genesis bump; land 1-7 inert first; optionally bump the `config.rs` genesis fingerprint so old nodes disconnect rather than fork. |
| R2 | Three-vec lockstep — `genesis_host`/`restore_host`/`sync_all_modules` + `MODULE_IDS` must change together or a restarted/synced node forks. | 8 | Single atomic PR; the restart + sync-only e2e legs (Phase 9) are the guard; keep the existing "keep in sync with genesis_host" comments. |
| R3 | Two stamping seams (node live `node/lib.rs:972`, recovery replay) must stamp the SAME pure `effective_version(height)` over committed end-of-`H−1` state. | 2, 6 | Both call the single `Host::effective_version`; never stamp the raw stored `current_version` (off-by-one fork, spec l.330-340). |
| R4 | Orchestrator `arm_verdict` vs module `Advance` must be byte-identical predicates or the engine flips while the module aborts. | 1, 6 | One shared pure predicate/semantics (same subset check, same frozen boundary valset, same `>= activation_height`); cross-checked by arm/abort tests + the module's `effective_version==reconciled` test. |
| R5 | Never-hashed regression — `boundary_version`/`upgrade_verdict`/`protocol_version`/`active_version` must never enter a root/manifest preimage. | 2, 5, 6 | Doc-comment each field; the Phase-2 app-hash-invariance test is the standing guard (`global_root` folds only `(id, root())`). |
| R6 | Manifest format bump breaks old readers (recovery `c.done()?`, statesync `expect_empty`). | 4 | Mandatory schema-tag / tolerant-tail decode on the recovery side (on-disk checkpoints outlive the process); statesync mixed-binary acceptable only under the Phase-8 stop-the-world retrofit. |
| R7 | Forge default-branch — `dev`'s forge already uses `compose_root`; defaulting `active_version` to a legacy single-head would MOVE the current root (not inert). | 5, 8 | Make the baseline decision explicit before Phase 8; the Phase-5 inert default MUST equal whatever baseline Phase 8 commits to (cleanest: default = current `compose_root`, demonstrate with a fresh higher `to_version`). |
| R8 | Governance app-hash continuity — new `GovAction` variants must take fresh tags 3/4; renumbering 0/1/2 forks existing proposals. | 3 | The byte-identical-encode test over tag-0/1/2 state. |
| R9 | Two-authority hazard — governance must NOT duplicate the module's monotonicity/min-lead/at-most-one gating. | 3 | Door-check limited to non-empty name; the module's `Err` fails Execute atomically (`rejected_followup_fails_execute_atomically`). |
| R10 | `SignalReady` spam / forgeable origin. | 7 | Local `(name,to_version)` dedupe + module idempotence; validators-and-current-member only; rely on the ordered lane's authenticated `Origin::External(pubkey)`. |
| R11 | `MIN_UPGRADE_LEAD` split-brain — appears module-side and driver-side and must be `>= CUTOVER_DELAY` or `H` lands inside an armed cutover window. | 1, 6 | Derive both from the one referenced `CUTOVER_DELAY` constant; test the min-lead rejection. |
| R12 | Env/BlockContext field sweep (~80 sites) — a missed literal is a compile error. | 2 | Add `Default`/constructor helpers; mechanical sweep in the one atomic PR. |
| R13 | Demonstrator gating — a real `root()` flip needs forge v2 + `MAX_PROTOCOL_VERSION=2`. | 9 | Phases 1-8 prove schedule/arm/abort without it; Phase 9 adds the app-hash-continuity-across-a-changed-root assertion. |

## Open questions

1. **Forge baseline version number (R7).** `dev`'s forge `root()` already computes
   `compose_root`. Is the current multi-repo composition the *baseline* v1 (so the
   Phase-9 demonstrator schedules some genuinely-new v2 format), or do we want to
   reintroduce the legacy single-head as v1 and treat today's `compose_root` as the
   v2 activation? The latter contradicts "inert default = current behavior" and must
   ride Phase 8; the former needs a concrete new v2 format to demonstrate against.
   Decide before Phase 5 freezes the default branch.
2. **External/query height-0 read paths.** `Host::query` and `ReadOnlyQueryCtx`
   hardcode height 0, so version-gated query projections read baseline logic
   regardless of the tip. Not a consensus issue (queries are unhashed) but a
   read-correctness wart — do we stamp the current effective_version for external
   reads, or document reads as baseline-format?
3. **`commitment` on `SignalReady`.** Phase 1 keeps it `None` (presence only) to
   avoid per-node build metadata diverging the module root. If we later want the
   defense-in-depth new-app-hash commitment, it must be a deterministic function of
   agreed target state, not per-node build bytes — a follow-up design question.
4. **Epoch-scoping the readiness signal.** The spec floats invalidating stale
   readiness promises on the teardown-respawn so validators re-affirm on the new
   binary (mitigating a signaled-then-downgraded node). Not in this plan's base
   scope; decide whether to add it before Phase 9.
