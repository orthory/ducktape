# `directory` → `tasks` Port Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Retire `directory` from the genesis module set (20 → 19) by moving every write/read tenant that uses it — seven process e2e suites, `--dev-demo`, one demo shell script — onto `tasks`, then dropping the topology row and re-pinning the genesis root in one flag-day commit.

**Architecture:** Two PRs from one worktree, sequentially. **PR A** (`port/directory-to-tasks`, base `dev`) moves the tenant and is fully green on today's 20-module set — `tasks` is already in `PRODUCTION` (`crates/topology/src/lib.rs:163`) and already ships an index guest (`crates/noded/src/index.rs:33,43`), so nothing about the module set changes yet. **PR B** (`port/directory-flag-day`, branched from A, base = A's branch; GitHub retargets it to `dev` when A merges) is the flag day: the topology row, the count pins, and `GENESIS_ROOT_HASH` move together in one indivisible commit, then the full e2e re-run and the prose sweep.

**Tech Stack:** Rust; `tasks` module (`crates/modules/apps/tasks`), `topology` table, `bin/node` process e2e harness (`bin/node/tests/common`), qmdb-backed `Backing::Store`.

**Spec:** `docs/superpowers/specs/2026-08-27-live-upgrade-design.md` — the Ruling-10 amendment at :195-200 is a standing forward commitment ("porting that lane to another indexed tenant is a follow-up; `directory` leaves with it") that this plan discharges. There is no separate spec document; that amendment plus the rulings below are the authority.

## Global Constraints

- **The `directory` CRATE stays.** Only its membership in the genesis set is retired. `crates/examples/greeter/src/lib.rs:22` hard-binds `directory: "directory"`, `crates/kernel/host/tests/{cross_module.rs:18,wasm_cutover_parity.rs:18}` `include_bytes!` its fixture wasm, and ~15 kernel/consensus/recovery files plus `bin/node/src/main_tests.rs` construct `Directory::new`. Keep it in `Makefile:216` `BUILDER_MODULES` and keep the committed component (`vaults` is the standing precedent for a component-shipping crate outside the universe). **Zero edits** to `crates/kernel/**`, `crates/examples/**`, `bin/node/src/main_tests.rs`, `bin/node/tests/network_joiner_full.rs`.
- **`TaskMsg::CreateTask` is not an upsert.** It validates and REFUSES a duplicate (`crates/modules/apps/tasks/src/task_board.rs:77-80`). That rejection is ISOLATED, not fatal: the op's stage rolls back and it is recorded `Rejected` while the block still seals (`crates/kernel/host/src/lib.rs:237`, `:283`) — so a duplicate silently never applies, and the confirm loop that waits for it spins to its timeout or passes VACUOUSLY on the surviving row. Every ported op must use a unique id. It also rejects: an empty id, an id over `MAX_TASK_ID` (256 bytes), an id containing `\x1f`, an empty title (`sdk::validate_id` + `require_non_empty`), and a record over 1 MiB − 4 KiB.
- **`TaskQuery` has exactly one variant, `List`** (`interface.rs:50-53`), returning the whole board. Every `dir_value` point read becomes list-then-find. The `Option`-shaped point lookup exists only on the derived index tier (`index.rs:118-120`) and is unreachable from an execute-path `ctx.query`.
- **Every payload gains the `WorkMsg::Task` envelope** (`interface.rs:245-247`). On the wire: `{"task":{"create_task":{"task_id":…,"title":…}}}`; a query is `{"task":"list"}`.
- **`GENESIS_ROOT_HASH` (`bin/node/src/host_state.rs:665-666`) moves exactly once, in PR B's flag-day commit**, together with every count pin. It must not move in PR A. `bin/simnode/tests/topology_set.rs:23` `DEFAULT_GENESIS_ROOT_HASH` and `bin/noded/tests/daemon_e2e.rs:462` do NOT move — `SIM_BASE` (`topology:180-196`) and `SIM_VALSET` (:205) never contained `directory`.
- **No migration code.** CLAUDE.md: zero live networks, no compat path. A descriptor-shaped workspace fails closed after the flag day with a named refusal (`compose.rs:110-125`: `code_hashes must key exactly the selection's wasm modules: … extra ["directory"]`); a dev-shaped workspace auto-adapts its hashes but its journal is pre-flag-day. Wipe-and-re-init is the whole story — say it in PR B's body, write nothing.
- House rules: one `match` per discriminant, named predicates, early return; `tracing` in node code (`info!` at most once per boot), `println!` only for CLI stdout; tests wait on events, never on time; only touched code formatted (never `cargo fmt --all`); per-crate lints `cargo clippy -p <crate> --tests --no-deps`.
- Host defect: rustc dies randomly here (incremental dep-graph ICE, DWARF SIGSEGV, corrupt-rlib `E0046` storms, `rust-lld` segfault). Rerun with `CARGO_INCREMENTAL=0`; a corrupt rlib → `cargo clean -p <crate>`; one cargo job at a time; never record an env prefix in the repo.

## Rulings (decided before execution; do not re-litigate)

| # | Question | Ruling |
|---|---|---|
| D1 | `restart_e2e` loses the Map-substrate limb (`directory`=`Backing::Map`, `tasks`=`Backing::Store`; a Store tenant never installs a checkpoint snapshot — `crates/noded/src/compose.rs:244-246`) | **Accept the loss, rewrite the comment honestly** (user, 2026-08-28). The snapshot-install→replay limb stays covered at kernel level by `crates/kernel/recovery/tests/restart_replay.rs:231-239`, which the surviving crate keeps green with zero edits. What the process e2e loses is that limb *in a live multi-process node*. |
| D2 | Does the `directory` crate go too? | **No — out of scope.** Deleting it is a separate, ~3× project (retarget/delete `greeter`, rebuild or delete the fixture component, rewrite ~15 fixture hosts onto `Tasks::new`, repoint `ops/wasm-repro-check.sh:24`). |
| D3 | `cluster_e2e`'s filler loop (~180 writes over the converge window) now makes every poll decode a growing board | **Keep the cadence.** ~180 tasks per decode is trivial; `task_board.rs:36-41`'s ~4k ceiling is far away. Ids stay unique (`cutover-filler-{n}`). |
| D4 | Existing workspaces on disk | **Wipe-and-re-init, noted in PR B's body, no code.** The live `p2p.ducktape` coordinator only breaks when someone deploys a post-flag-day binary to it — that re-init is a separate outward-facing op, asked for at deploy time, not part of this PR. |
| D5 | One PR or two? | **Two.** PR A (T1–T5) is green on the 20-module set; PR B (T6–T8) is the flag day, whose whole diff is a topology row plus four constants plus prose — small and obviously atomic to a reviewer. |

## File Structure

**PR A** — tenant swap only, no module-set change:

| File | Change |
|---|---|
| `bin/node/tests/{cluster,invite,large_file,sentry,statesync_fail_closed}_e2e.rs` | mechanical helper + literal swap (T1) |
| `bin/node/tests/live_admission_e2e.rs` | swap + three untyped sites the compiler cannot see (T2) |
| `bin/node/tests/restart_e2e.rs` | swap + D1 comment rewrite + one anti-vacuity assert (T3) |
| `bin/node/src/validator/engine.rs`, `bin/node/src/validator/run/drain.rs`, `bin/node/examples/demo-invite.sh` | the entire non-test production surface (T4) |

**PR B** — the flag day:

| File | Change |
|---|---|
| `crates/topology/src/lib.rs` | delete the `directory` row + PRODUCTION entry, `20`→`19`, membership pin, Map list (T6) |
| `bin/node/src/host_state.rs` | `GENESIS_ROOT_HASH` + three doc counts + the sim exclusion list (T6) |
| `bin/node/src/constants.rs`, `bin/node/Cargo.toml` | doc count; `directory` dep → `[dev-dependencies]` (T6) |
| `docs/**`, `README.md`, `Cargo.toml`, `crates/noded/src/lib.rs`, `skills/module-dev/SKILL.md` | prose sweep + discharge the Ruling-10 commitment (T8) |

## The shared swap (every T1–T3 task transcribes this)

```rust
// BEFORE (per-suite helpers; each suite carries its own copy)
use directory::{DirMsg, DirQuery, DirReply, decode_reply, encode_msg, encode_query};

fn dir_set(key: &str, value: &str) -> Vec<u8> {
    encode_msg(&DirMsg::Set { key: key.into(), value: value.into() })
}
fn dir_value(cluster: &Cluster, idx: usize, key: &str) -> Option<String> {
    let reply = cluster.query(idx, "directory", &encode_query(&DirQuery::Get { key: key.into() }))?;
    match decode_reply(&reply).ok()? { DirReply::Value(v) => v }
}

// AFTER
use tasks::{TaskMsg, TaskQuery, TaskReply, decode_task_reply, encode_task_msg, encode_task_query};

/// a create for `task_id`; NOT an upsert — `tasks` refuses a duplicate id
/// (`task_board.rs:77-80`). The rejection is ISOLATED — stage rolled back, op
/// recorded `Rejected`, block still seals — so a duplicate silently never
/// applies. Every call site carries a fresh id.
fn task_create(task_id: &str, title: &str) -> Vec<u8> {
    encode_task_msg(&TaskMsg::CreateTask { task_id: task_id.into(), title: title.into() })
}
/// `tasks` has no point read on the consensus tier (`TaskQuery::List` is its
/// only variant), so a title lookup lists the board and finds the id.
fn task_title(cluster: &Cluster, idx: usize, task_id: &str) -> Option<String> {
    let reply = cluster.query(idx, "tasks", &encode_task_query(&TaskQuery::List))?;
    match decode_task_reply(&reply).ok()? {
        TaskReply::Tasks(tasks) => tasks.into_iter().find(|t| t.id == task_id).map(|t| t.title),
    }
}
```

Every literal `"directory"` used as a submit/query target or asserted as a module name becomes `"tasks"`. `Cargo.toml` for `bin/node` keeps `directory` in `[dependencies]` through PR A (T6 moves it).

---

### Task 1: The five mechanical suites

**Files:**
- Modify: `bin/node/tests/cluster_e2e.rs`, `bin/node/tests/invite_e2e.rs`, `bin/node/tests/large_file_e2e.rs`, `bin/node/tests/sentry_e2e.rs`, `bin/node/tests/statesync_fail_closed_e2e.rs`

**Interfaces:**
- Consumes: `tasks::{TaskMsg, TaskQuery, TaskReply, decode_task_reply, encode_task_msg, encode_task_query}`. **No `Cargo.toml` change is owed:** `tasks` is already `bin/node/Cargo.toml:60` under `[dependencies]`, and the comment at `:226-227` records that the integration tests see `[dependencies]` without a `[dev-dependencies]` re-listing.
- Produces: the `task_create` / `task_title` helper pair, copied per suite exactly as the suites carry `dir_set`/`dir_value` today (they are per-file, not in `common/`).

- [ ] **Step 1: Swap the helpers and imports in each of the five files**, transcribing the shared swap above. `statesync_fail_closed_e2e.rs:46` and `large_file_e2e.rs:235` call `encode_msg(&DirMsg::Set{..})` inline — those become `encode_task_msg(&TaskMsg::CreateTask{..})` inline.
- [ ] **Step 2: Swap every `"directory"` literal** used as a target, query subject, or asserted module name. In `cluster_e2e.rs` specifically:
  - `:265-276` the app-surface JSON becomes `{"target":"tasks","payload":{"task":{"create_task":{"task_id":"via-app-surface","title":"held"}}}}`, and the blob assertion at `:388-401` must equal that same JSON byte-for-byte.
  - `:353-357` `module=\"tasks\"`; `:366` `ducktape_index_height{module="tasks"}`; `:443-446` `index["module"] == "tasks"`.
  - `:226-249` the filler loop keeps unique ids (`cutover-filler-{n}`) — a repeat would be a hard refusal, not a no-op (D3).
- [ ] **Step 3: Run the gate**

Run: `cargo test -p node-bin --test cluster_e2e --test invite_e2e --test large_file_e2e --test sentry_e2e --test statesync_fail_closed_e2e`
Expected: PASS. These suites still boot the 20-module set; `tasks` is already a production tenant, so nothing about genesis changes.

- [ ] **Step 4: Commit**

```bash
git add bin/node/tests
git commit -m "test(e2e): the five mechanical suites write through the tasks tenant"
```

---

### Task 2: `live_admission_e2e` — the index/route/backfill surface

**Files:**
- Modify: `bin/node/tests/live_admission_e2e.rs`

**Interfaces:** consumes Task 1's helper shape (this suite calls `directory::encode_msg` fully qualified at nine sites: `:129,158,467,578,611,697,788,847,921`).

- [ ] **Step 1: Mechanical swap** at `:95`, `:124-177`, `:435`, `:461-487`, `:573-630`, `:692-717`, `:784-806`, `:843-866`, `:917-939` — including the hex payload at `:578` (`common::hex(&encode_task_msg(&TaskMsg::CreateTask{..}))`).
- [ ] **Step 2: The three sites `cargo check` CANNOT catch** (string-keyed JSON — this is the whole reason this suite is its own task):
  - `:659-671` `index_status["modules"]["directory"]` and `["backfilled"]["directory"]` → `["tasks"]`. A stale key here does not fail fast: `unwrap_or(0)` pins the watermark at 0 and the poll dies on a misleading 30 s timeout while `floor.is_none()` keeps passing.
  - `:675-689` the route becomes `GET /v1/index/tasks/ops?limit=100`.
  - `:688` the payload predicate becomes `r["payload"]["task"]["create_task"]["task_id"] == json!("pre-join")`. The envelope IS present in the row: `op_row_json` (`crates/noded/src/index.rs:222-226`) decodes the submitted payload bytes verbatim and `encode_task_msg` wraps in `WorkMsg::Task`.
- [ ] **Step 3: Note the assertion got stronger** in a one-line comment where the floor is asserted: `directory` had no index guest, so `explorer.rs:340-356` cleared the backfill floor for free (`folds == false`); `tasks` folds through a real mapper, so `floor.is_none()` now genuinely depends on the fold reaching the backfilled rows.
- [ ] **Step 4: Run the gate**

Run: `cargo test -p node-bin --test live_admission_e2e`
Expected: PASS (this suite is slow — several minutes).

- [ ] **Step 5: Commit**

```bash
git add bin/node/tests/live_admission_e2e.rs
git commit -m "test(e2e): live admission asserts the tasks tenant's index rows and route"
```

---

### Task 3: `restart_e2e` — the swap plus ruling D1

**Files:**
- Modify: `bin/node/tests/restart_e2e.rs`

- [ ] **Step 1: Mechanical swap** — helpers at `:18`, `:25-30`, `:32-41`, `:43-55` (`write_and_confirm` polls `task_title(...) == Some(title)`), the writes at `:139-143`, `:202-224`, `:244-293`, and the row assertions at `:105` and `:186-197` (`r["ops"][0]["target"] == "tasks"`).
- [ ] **Step 2: Rewrite the substrate comment at `:83-84`** — it currently says the suite writes through `directory` BECAUSE that state is in-memory canonical bytes that dies without recovery. Under ruling D1 that reason is gone. Replace with an honest statement, e.g.:

```rust
    // real state across the substrates this checkpoint has to cover. `tasks`
    // is Backing::Store: its state comes back from the REOPENED qmdb store,
    // not from a checkpoint snapshot (crates/noded/src/compose.rs:244-246),
    // so what this suite proves is the store lane plus the explorer-row
    // rebuild and the op-blob re-stage. the Map/snapshot-install limb is
    // covered at kernel level by
    // crates/kernel/recovery/tests/restart_replay.rs:231-239.
```

- [ ] **Step 3: Add the anti-vacuity assert** before the loop at `:186-197`:

```rust
    assert!(
        !filtered.is_empty(),
        "no explorer rows targeted `tasks` — the poll above and this filter must name the same tenant"
    );
```

(The suite is not vacuous today — the poll at `:100-112` filters on the same string and fails closed at 30 s — but it would be if exactly one of the two sites were ported. One line makes that impossible.)

- [ ] **Step 4: Run the gate**

Run: `cargo test -p node-bin --test restart_e2e`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add bin/node/tests/restart_e2e.rs
git commit -m "test(e2e): restart writes through tasks; the map-substrate limb is kernel-covered"
```

---

### Task 4: `--dev-demo` and the demo script — the entire non-test production surface

**Files:**
- Modify: `bin/node/src/validator/engine.rs` (`:14`, `:255-276`), `bin/node/src/validator/run/drain.rs` (`:10`, `:890-918`), `bin/node/examples/demo-invite.sh` (`:103-108`)

- [ ] **Step 1: The seed** — `engine.rs:14` → `use tasks::{TaskMsg, encode_task_msg};`, and `:267-276`:

```rust
                    target: "tasks".into(),
                    payload: encode_task_msg(&TaskMsg::CreateTask {
                        task_id: format!("k{n}"),
                        title: format!("node-{n}"),
                    }),
```

**Keep the `dev_demo && resumed.is_none()` guard at `:267` verbatim** — it is exactly what makes the now-non-idempotent op safe; a re-seed of `k{n}` would turn an applied block into a rejected one.
- [ ] **Step 2: Rewrite the comment at `:255-266`** — the order-independence claim survives (each task is its own `t/{id}` record plus a sorted `t#` set, `task_board.rs:90-96`) but its stated reason changes, and `created_at`/`updated_at` now come from `ctx.env().consensus_time`.
- [ ] **Step 3: The dump** — `drain.rs:10` → the tasks codecs; `:896-918` collapses the `for k in 0..expected` loop into ONE query:

```rust
    let reply = node.host().query("tasks", &encode_task_query(&TaskQuery::List));
```

then iterate `TaskReply::Tasks`. **Keep `:890-895` (`converged root_hash=`) byte-identical** — `live_admission_e2e:38` waits on that marker.
- [ ] **Step 4: The demo script** — `demo-invite.sh:103-108`: `set_op` → `{"task":{"create_task":{"task_id":"ceremony","title":"two members, zero seeds"}}}`, `get_q` → `{"task":"list"}`, both targets → `tasks`. The existing grep for the title still matches inside the `List` reply.
- [ ] **Step 5: Run the gates**

Run: `cargo clippy -p node-bin --tests --no-deps` — expect clean.
Run: `bash bin/node/examples/demo-invite.sh` — expect the ceremony to complete.

- [ ] **Step 6: Commit**

```bash
git add bin/node/src/validator bin/node/examples/demo-invite.sh
git commit -m "feat(dev-demo): the seeded demo writes tasks, and the drain dump lists them"
```

---

### Task 5: Ship PR A

- [ ] **Step 1: Full gate sweep** — paste tails with exit codes (`${PIPESTATUS[0]}`):
  - `cargo test -p node-bin --tests`
  - `cargo clippy -p node-bin --tests --no-deps`
  - `cargo check --workspace --all-targets`
- [ ] **Step 2: Confirm the module set did NOT move** — `git diff dev..HEAD -- crates/topology bin/node/src/constants.rs bin/node/src/host_state.rs` must be EMPTY. `GENESIS_ROOT_HASH` moving in PR A is a defect.
- [ ] **Step 3: Push and open the PR**

```bash
git push -u origin port/directory-to-tasks
gh pr create --base dev --title "test(e2e): the directory tenant moves to tasks (no module-set change)"
```

Body: what moved and why (the Ruling-10 commitment), that the module set is untouched so no root moves here, the `CreateTask`-is-not-an-upsert consequence and where uniqueness is guaranteed, the `List`-only read shape, ruling D1 with the kernel-coverage citation, and that PR B is the flag day. Claude Code footer. **Do NOT merge.**

---

### Task 6: THE FLAG DAY — one commit, module set plus every pinned constant

Branch first: `git checkout -b port/directory-flag-day` (same worktree, on top of PR A's branch).

**Files:** `crates/topology/src/lib.rs`, `bin/node/src/host_state.rs`, `bin/node/src/constants.rs`, `bin/node/Cargo.toml`

Seven edits, indivisible — the pinned-hash test fails until the constant moves with the set:

- [ ] **Step 1: `crates/topology/src/lib.rs`** — (1) delete the `ModuleSpec { id: "directory", … }` row at `:129` (mandatory, not optional: `universe_and_selections_cover_each_other` at `:285-300` asserts universe == union of selections); (2) delete the `PRODUCTION` entry at `:170`; (3) `:235` `20` → `19`, both the value and the message; (4) `:243` drop `"directory",` from the sorted membership pin; (5) `:361` `["directory", "runs"]` → `["runs"]`; (6) `:147` doc `(20)` → `(19)`.
- [ ] **Step 2: `bin/node/src/host_state.rs`** — recompute `GENESIS_ROOT_HASH` at `:665-666`. Do NOT guess it: run the gate, and take the new value from the assertion failure, which prints it (`:1013-1022`). Also `:685` `20-module` → `19-module` and `:980` drop `directory` from the sim exclusion list.
- [ ] **Step 3: `bin/node/src/constants.rs:118`** `today's 20` → `today's 19`. There is NO array-length literal anywhere: `MODULE_IDS` (`:124`) is a bare alias of `topology::PRODUCTION`.
- [ ] **Step 4: `bin/node/Cargo.toml:35`** — move `directory = { workspace = true }` to `[dev-dependencies]`; it is still needed by `main.rs:111` (`#[cfg(test)]`) and `main_tests.rs`. Keep `crates/examples/directory` in `Makefile:216` `BUILDER_MODULES` and keep the committed fixture wasm.
- [ ] **Step 5: Run the gate**

Run: `cargo test -p topology && cargo test -p node-bin --lib host_state:: && cargo test -p node-bin --test workspace_registry_cli`
Expected: the first run FAILS at the pinned-hash assertion and prints the new root; paste that failure as evidence, set the constant, re-run to PASS.

- [ ] **Step 6: Commit** (one commit, all four files)

```bash
git add crates/topology bin/node/src/host_state.rs bin/node/src/constants.rs bin/node/Cargo.toml
git commit -m "feat(genesis)!: directory leaves the module set (20 -> 19; genesis root moves)"
```

---

### Task 7: Full e2e re-run on the 19-module set

**Files:** none (verification task); fix whatever it surfaces.

- [ ] **Step 1: Re-run everything Tasks 1–4 touched**, plus the suites that never named `directory` but boot `PRODUCTION` nodes and therefore compose one fewer module and re-derive the descriptor: `coordinated_invite_cli`, `resident_submit_e2e`, `dispatch_e2e`, `module_upgrade_e2e`.

Run: `cargo test -p node-bin --tests`
Expected: PASS. This is where a missed count pin or a stale descriptor surfaces.

- [ ] **Step 2: Prove the daemons did NOT move**

Run: `cargo test -p noded-bin && cargo test -p simnode`
Expected: PASS with `bin/noded/tests/daemon_e2e.rs:462` and `bin/simnode/tests/topology_set.rs:23` UNTOUCHED — `SIM_BASE`/`SIM_VALSET` never contained `directory`. If either needs an edit, stop and report: that means the selection tables were wrong, not the pins.
- [ ] **Step 3: Commit** any fixes with a message naming what the flag day surfaced (no commit if nothing broke).

---

### Task 8: Prose sweep, spec discharge, ship PR B

**Files:** `docs/superpowers/specs/2026-08-27-live-upgrade-design.md`, `docs/records/architecture/wasm-module-authoring.md`, `README.md`, `docs/src/content/docs/en/{human,agent}/reference/repository-map.mdx`, `docs/src/content/docs/en/human/reference/implementation-status.mdx`, `docs/src/content/docs/en/human/modules/product-modules.mdx`, `Cargo.toml`, `crates/noded/src/lib.rs`, `skills/module-dev/SKILL.md`

- [ ] **Step 1: Discharge the standing commitment** — the live-upgrade spec's Ruling-10 amendment (`:195-200`) says porting the lane to another indexed tenant is a follow-up and that `directory` leaves with it. Record what actually happened: the tenant (`tasks`), 20 → 19, and the new root.
- [ ] **Step 2: The rest of the sweep** (Edit tool per hunk, no scripts) — `wasm-module-authoring.md:11-12` drop the "stays in `topology::PRODUCTION` as the e2e lane's write tenant" claim, keep "first wasm port / the template" (still true of the crate); `README.md:39` and `Cargo.toml:65` drop "(also bin/node's liveness canary)"; both `repository-map.mdx` files drop "registered in bin/node as the production canary" (they drift as a pair); `implementation-status.mdx:52` drop `directory` from the snapshot-modules list and `:77` follow `demo-invite.sh`'s new tenant; `product-modules.mdx:42` drop it; `crates/noded/src/lib.rs:455` doc comment and `:1145` the System-bucket test array (both cosmetic — the `_ => System` fall-through means the test passes either way, which is exactly why it is silent drift). Leave `docs/superpowers/plans/**` and the dated `2026-07-*` records alone: they are records.
- [ ] **Step 3: Earn one row in `skills/module-dev/SKILL.md`** — its flag-day table should say that a module joining or leaving `PRODUCTION` also gains or loses a per-module index database and, if it ships a mapper, an arm in `crates/noded/src/index.rs`.
- [ ] **Step 4: Verify no module-sense survivor**

Run: `grep -rn 'directory' README.md docs/src Cargo.toml crates/noded/src/lib.rs | grep -v 'create_dir\|read_dir\|storage_directory\|tracker_directory'`
Expected: every survivor is about the CRATE, never the module.

- [ ] **Step 5: Commit and ship**

```bash
git add -A
git commit -m "docs: directory is a fixture crate, not a genesis tenant"
git push -u origin port/directory-flag-day
gh pr create --base port/directory-to-tasks --title "feat(genesis)!: directory leaves the module set (20 -> 19)"
```

Body: the root move and its two independent causes (19 module roots; the lifecycle registry seed loses `sha256(directory.component.wasm)` — `compose.rs:56-60`); that no suite re-pins a root literal because every other root assertion is a cross-node or before/after equality; ruling D4 verbatim (descriptor-shaped workspaces fail closed with `extra ["directory"]` from `compose.rs:110-125` and must be re-inited — the live `p2p.ducktape` coordinator included, at deploy time); that the crate stays and why (D2). Note the base is PR A's branch and retargets to `dev` when A merges. Claude Code footer. **Do NOT merge.**
