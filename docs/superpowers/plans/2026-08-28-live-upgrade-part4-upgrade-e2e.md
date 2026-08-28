# Live Upgrade Part 4 — `module_upgrade_e2e.rs` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove, on a real three-validator cluster, that a module registered after genesis and then live-swapped keeps its state across a crash restart and reaches a statesynced joiner — spec §5, which is the acceptance test for §1's "admitted modules across restart and statesync".

**Architecture:** One new process e2e suite, `bin/node/tests/module_upgrade_e2e.rs`, built on the harness (`tests/common/mod.rs`) and on the module-verb helpers Part 3 wrote in `tests/module_cli.rs`, which move to `tests/common/module_verbs.rs` so both suites share them. The suite runs the operator verbs through the real binary (`run_verb`), drives state through the rpc (`submit`/`query`), and synchronizes on the node's own events (`await_committed`, log markers). Anything steps 6–7 expose is a §1 defect in `bin/node/src/host_state.rs` and is fixed in this PR.

**Tech Stack:** Rust integration tests; `Cluster` harness; `governance` wire types for the joiner's admission; the kernel's `hello` / `hello-replacement` wasm fixtures.

**Spec:** `docs/superpowers/specs/2026-08-27-live-upgrade-design.md` §5 (and §1 "Admitted modules across restart and statesync", §4 for the verbs).

## Global Constraints

- No compat/legacy/versioning code (CLAUDE.md).
- Tests wait on events, never on time: `Cluster::await_committed` (wakes on the ws block feed) and `wait_marker`; no `std::thread::sleep`, no `poll_until` in NEW code (the harness's `poll_until` exists in older suites; this suite does not add to it).
- House rules: named predicates, early return, one `match` per discriminant, no boolean steering; only touched code formatted (never `cargo fmt --all`).
- Node code (if a fix is needed in `host_state.rs`) uses `tracing`, never `println!`; per-op events are `debug`.
- Gates: `cargo clippy -p node-bin --tests --no-deps` clean; `cargo check --workspace --all-targets` clean; `cargo test -p node-bin --test module_cli -- --test-threads=1` stays green (the helpers move).
- Cluster suites are plain `#[test]` + `let _guard = common::serial();` — `make test`'s first pass runs every `bin/node/tests/*.rs`; the `--ignored` lane is the bin's unit tests only (`Makefile:165-188`). **Spec §5's "`#[ignore]` like every cluster suite" is wrong; do not `#[ignore]`.**
- `--after 60` (`AFTER` in `module_verbs.rs`), not the spec's `--after 10`: three sequential CLI runs must clear `height + MIN_SWAP_LEAD` at execute time and the matcher's `deciding == 1` must stay exact on a loaded box (Part 3, `module_cli.rs:154-161`). The chain ticks one idle block per second (`consensus::BLOCK_TIME = 1 s`).
- Host defect: rustc may die in its incremental dep-graph decode or a DWARF SIGSEGV on the `ducktape` test target — rerun with `CARGO_INCREMENTAL=0`; never record an env prefix in the repo. Run cargo jobs one at a time.

## Corrections to spec §5 (rulings; the spec is updated in Task 5)

| Spec says | Reality (cited) | Ruling |
|---|---|---|
| step 6 waits "the `restart replayed the journal` marker" | no such line; the boot marker is `recovered root_hash=` (`bin/node/src/validator/boot.rs:134-144`, fields `replayed`, `already_on_disk`, `rolled_forward`) | wait `recovered root_hash=`; assert it equals the pre-crash status `root_hash` and that no `genesis root_hash=` marker appears (`restart_e2e.rs:112-128` template) |
| step 7 `spawn_joiner(4)`, sync-only | `spawn_joiner` (`common/mod.rs:1171-1230`) spawns a LIVE uninvited node and does not push `service_kinds`/`services`/`daemons`, so `config_path`/`run_sync_only`/`kill` on its index panic out of bounds; statesync is fail-closed for a peer with no committed standing (`statesync_fail_closed_e2e.rs:75-90`) | declare the joiner up front: `Cluster::new(&[1, 2, 3, 4], &[1, 2, 3])` (idx 3 = id 4, not spawned), admit `Cluster::identity(4)` through governance `AddValidator` (`cluster_e2e.rs:182-251` template), wait the epoch-1 cutover, then `run_sync_only(3, ..)` and compare its `synced root_hash=` with the founders' status root (`cluster_e2e.rs:494-503`) |
| step 7 `count == 101` on the joiner | `--sync-only` binds no rpc/http (`common/mod.rs:139-143`), so `Cluster::query(3, ..)` cannot answer a sync-only run | after the sync-only run, `spawn(3)` LIVE over the synced storage (it is an epoch-1 validator), wait `recovered root_hash=`, then `await_committed(3, ..)` on `count == 101` — this also exercises the restore path over statesynced storage |
| step 1 "wait admitted/serving" | Part 3's founders builder waits `converged root_hash=`, `module-code plane: overlay stream bound`, `peer handshake COMPLETE` per node | reuse it (`spawn_founders`, Task 1) |
| (unstated) checkpoint cadence | with the default cadence a checkpoint may land after the swap, so the restart would restore hello from the checkpoint instead of replaying it | pin `checkpoint_blocks = 100000` (`restart_e2e.rs:79`) so the restart REPLAYS the register, the swap and both `inc`s from the journal — the hard path (`admitted_restore_snapshot` → empty → replay, `host_state.rs:316-325`) is the one under test |

---

### Task 1: Share the module-verb e2e helpers

**Files:**
- Create: `bin/node/tests/common/module_verbs.rs`
- Modify: `bin/node/tests/common/mod.rs` (add `pub mod module_verbs;` next to the other `pub mod`/`pub use` lines at the top)
- Modify: `bin/node/tests/module_cli.rs` (delete the moved items, import them)

**Interfaces:**
- Produces (all `pub`, all doc-commented, bodies moved VERBATIM from `module_cli.rs:35-161` except where noted):
  - `pub const AFTER: &str = "60";`
  - `pub fn fixture(id: &str) -> String`
  - `pub fn active_hash(cluster: &Cluster, idx: usize, id: &str) -> Option<String>`
  - `pub fn sha256_hex(path: &str) -> String`
  - `pub fn spawn_founders(mut cluster: Cluster) -> Cluster` — the body of `three_validators()` from `cluster.wireguard = true;` on, taking the cluster the caller built (so a suite can declare extra peer ids). The doc comment moves with it. `module_cli.rs` keeps a two-line `fn three_validators() -> Cluster { spawn_founders(Cluster::new(&[1, 2, 3], &[1, 2, 3])) }`.
  - `pub fn run_on_each(cluster: &Cluster, verb: &[&str]) -> Vec<(bool, String)>` — unchanged (runs on idx 0..3).
  - `pub fn assert_ceremony_scheduled(runs: &[(bool, String)], id: &str)` and `fn outputs(..)` (private helper stays in the new file).
- `module_cli.rs` keeps: `ducktape()`, the no-node status test, `three_validators()` (two lines), the three cluster tests, `assert_no_proposals`.

- [ ] **Step 1: Create `bin/node/tests/common/module_verbs.rs`** with the items above. Header doc: `//! the module-verb e2e helpers shared by module_cli.rs and module_upgrade_e2e.rs.` Imports: `use std::time::Duration; use super::{Cluster, FIXTURES};`.
- [ ] **Step 2: Register it** — in `bin/node/tests/common/mod.rs` add `pub mod module_verbs;` (with the other module declarations at the top of the file).
- [ ] **Step 3: Cut the items out of `module_cli.rs`** (Edit per hunk) and add `use common::module_verbs::{active_hash, assert_ceremony_scheduled, fixture, run_on_each, sha256_hex, spawn_founders, AFTER};`. Replace `three_validators()`'s body with `spawn_founders(Cluster::new(&[1, 2, 3], &[1, 2, 3]))`.
- [ ] **Step 4: Verify** — `cargo test -p node-bin --test module_cli status_against_no_node -- --test-threads=1` (compiles the suite; the no-node case runs in seconds) and `cargo clippy -p node-bin --tests --no-deps` (an unused-import or dead-code warning here means something was left behind). Expected: green, 0 warnings. The full `module_cli` lane runs in Task 5.
- [ ] **Step 5: Commit** — `refactor(test): the module-verb e2e helpers move to common/module_verbs.rs`.

---

### Task 2: The swap-and-count suite (spec steps 1–5)

**Files:**
- Create: `bin/node/tests/module_upgrade_e2e.rs`

**Interfaces:**
- Consumes: Task 1's helpers; `Cluster::{new, spawn, submit, query, status, await_committed, config_file, run_verb}`; `common::serial`.
- Produces (private, used by Tasks 3–4 in the same file): `fn founders_and_declared_joiner() -> Cluster`, `fn count(cluster: &Cluster, idx: usize) -> Option<u64>`, `fn root_hashes_agree(cluster: &Cluster, idxs: &[usize]) -> Option<String>`, `fn inc_and_confirm(cluster: &Cluster, submit_on: usize, expect: u64)`, `fn register_and_activate(cluster: &Cluster)`, `fn update_and_activate(cluster: &Cluster)`, `const FINALIZE: Duration`.

- [ ] **Step 1: Write the suite through step 5** (`bin/node/tests/module_upgrade_e2e.rs`):

```rust
//! spec §5: a module registered after genesis, live-swapped, then carried
//! across a crash restart and to a statesynced joiner — the acceptance test
//! for §1's "admitted modules across restart and statesync".
mod common;

use std::time::Duration;

use common::module_verbs::{
    active_hash, assert_ceremony_scheduled, fixture, run_on_each, sha256_hex, spawn_founders,
    AFTER,
};
use common::Cluster;

/// a query or status read that should already be true lands within a block
/// or two; this is the budget for a ws-block-fed wait on it.
const FINALIZE: Duration = Duration::from_secs(60);
/// a swap activates `AFTER` idle blocks after the deciding ballot.
const ACTIVATE: Duration = Duration::from_secs(180);

/// three founders (3-of-3) plus a DECLARED fourth peer (idx 3 = id 4) that
/// is not spawned: statesync is fail-closed for a peer with no committed
/// standing, and the harness's joiner helpers key on the index, so the
/// joiner must exist in the cluster layout from the start.
fn founders_and_declared_joiner() -> Cluster {
    let mut cluster = Cluster::new(&[1, 2, 3, 4], &[1, 2, 3]);
    // every sealed block stays in the journal-replay window: the restart
    // must REPLAY the register, the swap and both incs, not restore hello
    // from a checkpoint that happened to land after them.
    cluster.extra_toml.push("checkpoint_blocks = 100000".into());
    spawn_founders(cluster)
}

/// hello's query is "any bytes → the counter as LE u64"; `None` while the
/// node cannot answer for it yet (not serving, or the module not active).
fn count(cluster: &Cluster, idx: usize) -> Option<u64> {
    let reply = cluster.query(idx, "hello", b"")?;
    let bytes: [u8; 8] = reply.get(..8)?.try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}

/// the composite root every listed node reports, when they all agree.
fn root_hashes_agree(cluster: &Cluster, idxs: &[usize]) -> Option<String> {
    let hashes: Vec<String> = idxs
        .iter()
        .map(|&idx| cluster.status(idx)["root_hash"].as_str().map(str::to_string))
        .collect::<Option<_>>()?;
    let all_same = hashes.iter().all(|h| *h == hashes[0]);
    all_same.then(|| hashes[0].clone())
}

/// `inc` on one node, the new count readable on every founder.
fn inc_and_confirm(cluster: &Cluster, submit_on: usize, expect: u64) {
    cluster.submit(submit_on, "hello", b"inc");
    for idx in 0..3 {
        let seen = cluster.await_committed(
            idx,
            &format!("hello count == {expect} on node {idx}"),
            FINALIZE,
            || count(cluster, idx).filter(|c| *c == expect),
        );
        assert_eq!(seen, expect);
    }
}

fn register_and_activate(cluster: &Cluster) {
    let runs = run_on_each(
        cluster,
        &["module", "register", "hello", &fixture("hello"), "--after", AFTER],
    );
    assert_ceremony_scheduled(&runs, "hello");
    let first = sha256_hex(&fixture("hello"));
    for idx in 0..3 {
        let seen = cluster.await_committed(idx, "hello active", ACTIVATE, || {
            active_hash(cluster, idx, "hello").filter(|h| *h == first)
        });
        assert_eq!(seen, first);
    }
}

fn update_and_activate(cluster: &Cluster) {
    let runs = run_on_each(
        cluster,
        &[
            "module",
            "update",
            "hello",
            &fixture("hello-replacement"),
            "--after",
            AFTER,
        ],
    );
    assert_ceremony_scheduled(&runs, "hello");
    let second = sha256_hex(&fixture("hello-replacement"));
    for idx in 0..3 {
        let seen = cluster.await_committed(idx, "hello swapped", ACTIVATE, || {
            active_hash(cluster, idx, "hello").filter(|h| *h == second)
        });
        assert_eq!(seen, second);
    }
}

#[test]
fn a_registered_module_survives_a_live_swap_a_restart_and_statesync() {
    let _guard = common::serial();
    let mut cluster = founders_and_declared_joiner();

    // 2. register hello on all three; 3. inc → 1 everywhere
    register_and_activate(&cluster);
    inc_and_confirm(&cluster, 1, 1);

    // 4. swap in the replacement (steps by 100); 5. inc → 101 everywhere,
    // and the composite root agrees across the founders
    update_and_activate(&cluster);
    inc_and_confirm(&cluster, 1, 101);
    let root_after_swap = cluster.await_committed(
        0,
        "founders' root-hashes to agree after the swap",
        FINALIZE,
        || root_hashes_agree(&cluster, &[0, 1, 2]),
    );

    // Task 3 continues here (step 6), then Task 4 (step 7)
    let _ = (&mut cluster, root_after_swap);
}
```

- [ ] **Step 2: Run it** — `cargo test -p node-bin --test module_upgrade_e2e -- --test-threads=1 --nocapture 2>&1 | tee /tmp/…; echo ${PIPESTATUS[0]}`. Expected: PASS in ~3–4 minutes (two ceremonies at `--after 60` + activation waits). If `count` never reaches 1: the submit reply carries the rejection — print `cluster.rpc(1, {"cmd":"submit",...})` once to see it (`ingress.rs:96-108` routes any target; a `Rejected` means the payload is not exactly `b"inc"`).
- [ ] **Step 3: Commit** — `test(node): module_upgrade_e2e — register, swap and count on three validators`.

---

### Task 3: The restart proof (spec step 6)

**Files:**
- Modify: `bin/node/tests/module_upgrade_e2e.rs` (extend the test after `root_after_swap`)
- Possibly modify: `bin/node/src/host_state.rs` (only if the step fails — see "If it fails")

**Interfaces:**
- Consumes: `Cluster::{kill, spawn, wait_marker, marker, status}`; `count`, `root_hashes_agree`.

- [ ] **Step 1: Replace the placeholder tail** with:

```rust
    // 6. crash node 2 (SIGKILL) and respawn it over the same storage. with
    // 3-of-3 the chain halts while it is down and resumes when it is back.
    // the boot must RECOVER — replaying hello's register, swap and both
    // incs from the journal — not re-run genesis.
    cluster.kill(2);
    cluster.spawn(2);
    let recovered = cluster.wait_marker(2, "recovered root_hash=", Duration::from_secs(120));
    let recovered_hash = recovered.split_whitespace().next().expect("recovered hash");
    assert_eq!(
        recovered_hash, root_after_swap,
        "node 2 recovered a different root than the founders' post-swap boundary"
    );
    assert!(
        cluster.marker(2, "genesis root_hash=").is_none(),
        "a restart must not re-run genesis"
    );
    let seen = cluster.await_committed(2, "hello count == 101 after restart", FINALIZE, || {
        count(&cluster, 2).filter(|c| *c == 101)
    });
    assert_eq!(seen, 101);
    let root_after_restart = cluster.await_committed(
        0,
        "founders' root-hashes to agree after the restart",
        FINALIZE,
        || root_hashes_agree(&cluster, &[0, 1, 2]),
    );
    assert_eq!(root_after_restart, root_after_swap, "a restart moved the state root");

    // Task 4 continues here (step 7)
```

- [ ] **Step 2: Run it** — same command as Task 2 Step 2. Expected: PASS. Note the boot marker's `replayed` field in node 2's log (`cluster.dir/node3.log` or wherever `spawn` writes it; `wait_marker` returns the rest of the line) — paste it in the report: it must show the replay covered the swap (a `replayed = 0` with the count still 101 means a checkpoint carried it — check `checkpoint_blocks` actually applied).
- [ ] **Step 3 — If it fails**: this is §1 incomplete, not the test. Root-cause in `bin/node/src/host_state.rs:270-306` (`restore_host` → `adopt_admitted_modules` `:352-386`, `admitted_restore_snapshot` `:316-325`, `BlobCodeSource` `:279`) and the replay of `RegisterModule`/`UpdateModule` activations through the lifecycle. Write the failing case as a unit test next to `host_state.rs:769-816` first, fix, then re-run the e2e. Keep the fix in its seam; `tracing::debug!(target: "ducktape::modules", ..)` for per-module events. Report the exact symptom and fix as its own commit `fix(node): <what the restart lost>`.
- [ ] **Step 4: Commit** — `test(node): module_upgrade_e2e — a crash restart replays the admitted module`.

---

### Task 4: The statesynced joiner (spec step 7)

**Files:**
- Modify: `bin/node/tests/module_upgrade_e2e.rs`
- Possibly modify: `bin/node/src/host_state.rs` / `bin/node/src/blob_fetch.rs` (only if the step fails)

**Interfaces:**
- Consumes: `governance::{encode_msg, encode_query, decode_reply, GovAction, GovMsg, GovQuery, GovReply, ProposalStatus}` (already a dev-dep of node-bin; `cluster_e2e.rs:1-30` shows the imports), `Cluster::{identity, submit, query, marker, await_committed, run_sync_only, spawn, wait_marker}`.
- Produces: `fn proposal_status(cluster, idx, id) -> Option<(ProposalStatus, usize)>` (copy of `cluster_e2e.rs:88-100`), `fn admit_validator(cluster: &Cluster, key: [u8; 32])`.

- [ ] **Step 1: Add the admission helper** (the ceremony as `cluster_e2e.rs:182-251` runs it, synchronized on the block feed instead of `poll_until`):

```rust
use governance::{
    decode_reply, encode_msg, encode_query, GovAction, GovMsg, GovQuery, GovReply, ProposalStatus,
};

fn proposal_status(cluster: &Cluster, idx: usize, id: &str) -> Option<(ProposalStatus, usize)> {
    let reply = cluster.query(
        idx,
        "governance",
        &encode_query(&GovQuery::Proposal {
            proposal_id: id.into(),
        }),
    )?;
    match decode_reply(&reply) {
        Ok(GovReply::Proposal(Some(view))) => Some((view.status, view.votes.len())),
        _ => None,
    }
}

/// node 0 proposes seating `key`, nodes 0+1 vote (2 of 3 = majority), node 1
/// executes; the passing proposal emits the valset Join, and the founders
/// cross the epoch-1 cutover on their own idle blocks.
fn admit_validator(cluster: &Cluster, key: [u8; 32]) {
    const ID: &str = "admit-joiner";
    cluster.submit(
        0,
        "governance",
        &encode_msg(&GovMsg::Propose {
            proposal_id: ID.into(),
            action: GovAction::AddValidator { key },
            voting_period: 600_000,
        }),
    );
    cluster.await_committed(1, "admission proposal to open", FINALIZE, || {
        proposal_status(cluster, 1, ID).filter(|(s, _)| *s == ProposalStatus::Open)
    });
    let vote = encode_msg(&GovMsg::Vote {
        proposal_id: ID.into(),
        approve: true,
    });
    cluster.submit(0, "governance", &vote);
    cluster.submit(1, "governance", &vote);
    cluster.await_committed(1, "both ballots to land", FINALIZE, || {
        proposal_status(cluster, 1, ID).filter(|(_, votes)| *votes == 2)
    });
    cluster.submit(
        1,
        "governance",
        &encode_msg(&GovMsg::Execute {
            proposal_id: ID.into(),
        }),
    );
    cluster.await_committed(0, "admission to settle as Passed", FINALIZE, || {
        proposal_status(cluster, 0, ID).filter(|(s, _)| *s == ProposalStatus::Passed)
    });
    cluster.await_committed(0, "the epoch-1 cutover on every founder", ACTIVATE, || {
        let every_founder_cut_over =
            (0..3).all(|idx| cluster.marker(idx, "cutover complete: epoch 1").is_some());
        every_founder_cut_over.then_some(())
    });
}
```

If the cutover does not cross on idle blocks within `ACTIVATE` (cluster_e2e pushed `directory` fillers through it, `cluster_e2e.rs:222-248`, from before idle nops existed — `drain.rs:933-950`), submit one `hello`-free filler per probe call inside the `await_committed` closure exactly as cluster_e2e does (`directory` `DirMsg::Set`), and say so in the report; do not add a sleep.

- [ ] **Step 2: Replace the placeholder tail** with:

```rust
    // 7. seat the declared joiner, then let it statesync as a fresh resident:
    // every module — hello included, whose bytes it can only pull over the
    // blob plane — must compose the founders' root.
    admit_validator(&cluster, Cluster::identity(4));
    let root_before_sync = cluster.await_committed(
        0,
        "founders' root-hashes to agree before the sync",
        FINALIZE,
        || root_hashes_agree(&cluster, &[0, 1, 2]),
    );
    let (ok, log) = cluster.run_sync_only(3, Duration::from_secs(180));
    assert!(ok, "sync-only joiner failed:\n{log}");
    let synced = log
        .lines()
        .find_map(|l| l.split("synced root_hash=").nth(1))
        .expect("joiner printed a synced root-hash")
        .trim();
    assert_eq!(synced, root_before_sync, "joiner composed a DIFFERENT root-hash");

    // the joiner boots LIVE over the synced storage (a sync-only run binds
    // no rpc): it is an epoch-1 validator, so it recovers and serves, and
    // hello answers 101 from state it never executed.
    cluster.spawn(3);
    cluster.wait_marker(3, "recovered root_hash=", Duration::from_secs(120));
    let seen = cluster.await_committed(3, "hello count == 101 on the joiner", ACTIVATE, || {
        count(&cluster, 3).filter(|c| *c == 101)
    });
    assert_eq!(seen, 101);
    let root_with_joiner = cluster.await_committed(
        0,
        "all four root-hashes to agree",
        FINALIZE,
        || root_hashes_agree(&cluster, &[0, 1, 2, 3]),
    );
    assert_eq!(root_with_joiner, root_before_sync);
```

  Note `root_before_sync` vs `root_after_restart`: the admission ceremony changed governance/valset state, so the root moves between them — compare the joiner against the post-admission root only. If the live joiner's boot marker is not `recovered root_hash=` (a synced-then-booted node may log `promoted: validator at epoch 1` or `joiner mode:` first — `cluster_e2e.rs:220,251`, `replica/park.rs`), grep node 3's log for the marker it does print, wait on that, and record it in the report; `await_committed(3, ..)` needs node 3's ws feed, which exists once it serves.

- [ ] **Step 3: Run it** — same command; expected PASS in ~6–8 minutes total. Paste the joiner's `synced root_hash=` line and node 3's boot markers in the report.
- [ ] **Step 4 — If it fails**: §1's statesync half is incomplete. The seams: `sync_all_modules` `host_state.rs:454-640` (`FetchingCodeSource` `:585-590` pulls hello's bytes over the blob plane, sha256-checked in `blob_fetch.rs:297-334`; `adopt_admitted_modules` `:629-633`; the composite gate `"composed {} != manifest {}"`). The only failure log is `debug!(target: "ducktape::modules", .. "code blob unavailable")` (`blob_fetch.rs:326-331`) — run the joiner with `RUST_LOG=info,ducktape::modules=debug` in `cluster.env` to see it. Unit-test the failing shape first, fix in its seam, commit `fix(node): <what the sync lost>`.
- [ ] **Step 5: Commit** — `test(node): module_upgrade_e2e — a statesynced joiner composes the admitted module`.

---

### Task 5: Spec correction, gates, PR

**Files:**
- Modify: `docs/superpowers/specs/2026-08-27-live-upgrade-design.md` §5 — rewrite the seven steps to what the suite does (the corrections table above: `recovered root_hash=`, the declared joiner + `AddValidator`, sync-only then live boot for the count, `--after 60`, plain `#[test]`, `checkpoint_blocks` pinned), dated 2026-08-28.

- [ ] **Step 1: Spec edit** (Edit tool per hunk).
- [ ] **Step 2: Gates** — paste tails with exit codes (`${PIPESTATUS[0]}`):
  - `cargo clippy -p node-bin --tests --no-deps`
  - `cargo check --workspace --all-targets`
  - `cargo test -p node-bin --test module_cli -- --test-threads=1` (the moved helpers; 4/4)
  - `cargo test -p node-bin --test module_upgrade_e2e -- --test-threads=1` (1/1; record the time)
  - `cargo test -p node-bin --test cluster_e2e -- --test-threads=1` (untouched, but the harness module list changed — proves `common/mod.rs` still builds for the other suites)
- [ ] **Step 3: Commit docs, push, `gh pr create --base dev --title "test(node): module_upgrade_e2e — a post-genesis module across a live swap, a restart and statesync"`.** Body: spec §5 link + the corrections table; what the suite proves step by step with the observed markers (`recovered root_hash=` with its `replayed` count, `synced root_hash=`); any §1 fix it forced (or "none — §1 held"); timings; follow-ups (`spawn_joiner`'s index bookkeeping gap — `service_kinds`/`services`/`daemons` not pushed, `common/mod.rs:1171-1230`; the §5 count-on-joiner needs a live boot; `directory` retirement will need the filler, if used, moved to `tasks`). Claude Code footer. Do NOT merge.

---

## Self-review

- **Spec coverage:** §5 steps 1–5 → Task 2; step 6 → Task 3; step 7 → Task 4; "fixes anything it exposes in 2" → Tasks 3/4 "If it fails" steps; the doc reconciliation → Task 5. §1's restart and statesync claims are the two things under test.
- **Placeholders:** none — each step has its code; the two "If it fails" steps name the seams and the first move (unit test) because the defect, if any, is unknown by definition.
- **Type consistency:** `count -> Option<u64>` used with `.filter(|c| *c == expect)` in Tasks 2–4; `root_hashes_agree -> Option<String>` compared as `String` against `recovered_hash: &str` via `assert_eq!(&str, String)` — write `assert_eq!(recovered_hash, root_after_swap)` with `root_after_swap: String` and `recovered_hash: &str` (`PartialEq<String> for &str` holds); `run_sync_only -> (bool, String)`; `await_committed` returns the probe's `T`; `spawn_founders(Cluster) -> Cluster` consumed by both suites; `AFTER: &str` passed straight into the verb argv.
