# Compute-Aware Capability Scheduling — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Nodes announce numeric compute capacity (cores/mem_gb) opt-in (default OFF); runs carry explicit numeric demands; consensus assignment filters providers by total capacity; hosts admit by free capacity; podman enforces the limits on Linux.

**Architecture:** Extend the existing capability registry (consensus) with an open-set `resources: BTreeMap<String,u64>` per node, thread `demands` through the run→dispatch→saga→WorkSpec wire, filter saga rendezvous pools with a new demands-aware registry query, gate execution host-side with a local reservation ledger, and wrap the executor CLI spawn in a `SandboxBackend` (Direct | Podman) that turns demands into `--cpus/--memory`. No new crates.

**Tech Stack:** Rust (workspace crates: capability, saga, dispatch, dispatch-oracle, capability-host, runs, bin/node, bin/noded), TypeScript (app), podman rootless.

**Spec:** `docs/superpowers/specs/2026-07-12-compute-capability-sandbox-design.md`

## Global Constraints

- Work in a git worktree under `<primary-checkout>/.worktree/compute-capability-p1`, branched from `origin/dev`; deliver as a PR against `dev`.
- Build/lint through `ops/build-with.sh cargo ...`; per-crate lint gate is `ops/build-with.sh cargo clippy -p <crate> --tests --no-deps`.
- Never `cargo fmt --all`; format only touched code.
- `ops/build-with.sh cargo check -p files --no-default-features` must stay green (should be untouched by this plan).
- Cached cargo re-emits no warnings: `touch` a changed `.rs` before trusting a clippy/check pass.
- ~600-line soft cap per file; new logic goes in new focused files (`ledger.rs`, `sandbox.rs`, `host_resources.rs`).
- **FLAG DAY:** capability + saga snapshot/root encodings change → root-hash moves. Pre-existing networks must re-genesis. Say so in the PR body.
- **Behavior change:** `announce_capabilities` default flips ON→OFF (opt-in serving). Say so in the PR body.
- Wire compat: every new wire field uses `#[serde(default)]` so old JSON still decodes; Rust construction sites are fixed in the same task that adds the field (workspace must compile at every commit).
- Commit messages end with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.

---

### Task 1: capability interface — resources vocabulary + new queries

**Files:**
- Modify: `crates/system/capability/src/interface.rs`
- Modify (compile fix): `bin/node/src/validator/announce.rs:156` (Announce construction gains `resources`)

**Interfaces:**
- Consumes: existing `validate_tag`, `MAX_TAG_LEN`.
- Produces: `pub const MAX_RESOURCE_DIMS: usize = 16`; `pub fn validate_resources(&BTreeMap<String, u64>) -> Result<(), String>`; `CapabilityMsg::Announce { capabilities, resources: BTreeMap<String, u64> }`; `CapabilityQuery::CapableProviders { capability: String, demands: BTreeMap<String, u64> }`; `CapabilityQuery::Resources { node: Vec<u8> }`; `CapabilityReply::Resources(BTreeMap<String, u64>)`. Existing `Providers`/`Node`/`All` replies unchanged (zero TS churn).

- [ ] **Step 1: Write failing tests** (in `interface.rs` `#[cfg(test)]`, new module — the crate keeps tests near types):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn res(pairs: &[(&str, u64)]) -> BTreeMap<String, u64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn validate_resources_accepts_sane_maps_and_rejects_bad_ones() {
        assert!(validate_resources(&res(&[])).is_ok());
        assert!(validate_resources(&res(&[("cores", 8), ("mem_gb", 32)])).is_ok());
        // keys obey the ONE tag rule (charset/length), values must be non-zero
        assert!(validate_resources(&res(&[("Cores", 8)])).is_err());
        assert!(validate_resources(&res(&[("cores", 0)])).is_err());
        let too_many: BTreeMap<String, u64> =
            (0..=MAX_RESOURCE_DIMS).map(|i| (format!("d{i}"), 1)).collect();
        assert!(validate_resources(&too_many).is_err());
    }

    #[test]
    fn announce_without_resources_field_still_decodes() {
        // old wire JSON (pre-resources) must decode with an empty map.
        let old = br#"{"announce":{"capabilities":["codex"]}}"#;
        let CapabilityMsg::Announce { capabilities, resources } = decode_msg(old).unwrap();
        assert_eq!(capabilities, vec!["codex"]);
        assert!(resources.is_empty());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `ops/build-with.sh cargo test -p capability`
Expected: FAIL — `validate_resources` / `MAX_RESOURCE_DIMS` / `resources` field not found.

- [ ] **Step 3: Implement**

```rust
/// most dimensions one node may announce / one job may demand — a bound, not
/// a schema (dimensions are open-set data: "cores", "mem_gb", later "gpu").
pub const MAX_RESOURCE_DIMS: usize = 16;

/// the ONE rule for a resource map (announced capacity or demanded amounts):
/// bounded size, keys under the tag rule, values non-zero (zero means "don't
/// name the dimension").
pub fn validate_resources(resources: &BTreeMap<String, u64>) -> Result<(), String> {
    if resources.len() > MAX_RESOURCE_DIMS {
        return Err(format!(
            "too many resource dimensions: {} exceeds the {MAX_RESOURCE_DIMS} cap",
            resources.len()
        ));
    }
    for (key, value) in resources {
        validate_tag(key).map_err(|e| format!("resource dimension {key:?}: {e}"))?;
        if *value == 0 {
            return Err(format!("resource dimension {key:?} is zero (omit it instead)"));
        }
    }
    Ok(())
}
```

Extend the enums (note `use std::collections::BTreeMap;` at the top):

```rust
    Announce {
        capabilities: Vec<String>,
        /// announced numeric capacity (e.g. "cores", "mem_gb"). EMPTY for a
        /// direct-spawn node: tags-only, never matches a demands-carrying job.
        #[serde(default)]
        resources: BTreeMap<String, u64>,
    },
```

```rust
    /// every node that announced `capability` AND whose announced resources
    /// cover `demands` per dimension (absent dimension ≠ infinite). empty
    /// demands degrade to `Providers`.
    CapableProviders {
        capability: String,
        demands: BTreeMap<String, u64>,
    },
    /// the resource map a single node announced (empty if absent) — the
    /// announcer's idempotence read, beside `Node`.
    Resources { node: Vec<u8> },
```

```rust
    Resources(BTreeMap<String, u64>),
```

In `bin/node/src/validator/announce.rs` fix the construction (real values arrive in Task 8):

```rust
            payload: encode_msg(&CapabilityMsg::Announce {
                capabilities,
                resources: Default::default(),
            }),
```

- [ ] **Step 4: Run tests + workspace check**

Run: `ops/build-with.sh cargo test -p capability && ops/build-with.sh cargo check -p node`
Expected: PASS (capability lib.rs doesn't yet handle the new query variants — if `query` match is non-exhaustive it will fail to compile; in that case add temporary arms returning `Error::QueryUnsupported` and note Task 2 replaces them).

- [ ] **Step 5: Commit**

```bash
git add crates/system/capability/src/interface.rs bin/node/src/validator/announce.rs
git commit -m "feat(capability): resources vocabulary on the wire surface"
```

---

### Task 2: capability lib — registry stores resources; root/snapshot v2; capable-providers filter

**Files:**
- Modify: `crates/system/capability/src/lib.rs`

**Interfaces:**
- Consumes: Task 1's `validate_resources`, new query/reply variants.
- Produces: registry state is `BTreeMap<Vec<u8>, NodeEntry>` with `pub(crate) struct NodeEntry { tags: BTreeSet<String>, resources: BTreeMap<String, u64> }`; `snapshot()`/`install()`/`root()` cover resources; `query` answers `CapableProviders` and `Resources`.

- [ ] **Step 1: Write failing tests** (extend the existing `#[cfg(test)]` module; `announce()` helper gains a resources param):

```rust
    fn announce_with(tags: &[&str], resources: &[(&str, u64)]) -> Msg {
        Msg {
            target: "capability".into(),
            payload: encode_msg(&CapabilityMsg::Announce {
                capabilities: tags.iter().map(|t| t.to_string()).collect(),
                resources: resources.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            }),
        }
    }

    fn capable(c: &CapabilityRegistry, capability: &str, demands: &[(&str, u64)]) -> Vec<Vec<u8>> {
        let reply = futures::executor::block_on(c.query(&encode_query(
            &CapabilityQuery::CapableProviders {
                capability: capability.into(),
                demands: demands.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            },
        )))
        .unwrap();
        match crate::decode_reply(&reply).unwrap() {
            CapabilityReply::Providers(p) => p,
            other => panic!("expected Providers reply, got {other:?}"),
        }
    }

    #[test]
    fn resources_are_stored_queryable_and_move_the_root() {
        let mut c = ungated();
        let me = vec![30u8; 32];
        let mut ctx = TestCtx::external(&me);
        futures::executor::block_on(c.execute(&mut ctx, &announce_with(&["codex"], &[])))
            .unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        let tags_only_root = c.root();

        futures::executor::block_on(
            c.execute(&mut ctx, &announce_with(&["codex"], &[("cores", 8), ("mem_gb", 32)])),
        )
        .unwrap();
        futures::executor::block_on(c.commit_block()).unwrap();
        assert_ne!(c.root(), tags_only_root, "resources are part of the commitment");

        let reply = futures::executor::block_on(c.query(&encode_query(
            &CapabilityQuery::Resources { node: me.clone() },
        )))
        .unwrap();
        match crate::decode_reply(&reply).unwrap() {
            CapabilityReply::Resources(r) => {
                assert_eq!(r.get("cores"), Some(&8));
                assert_eq!(r.get("mem_gb"), Some(&32));
            }
            other => panic!("expected Resources reply, got {other:?}"),
        }
    }

    #[test]
    fn capable_providers_filters_per_dimension_and_absent_is_not_infinite() {
        let mut c = ungated();
        let big = vec![31u8; 32];
        let small = vec![32u8; 32];
        let bare = vec![33u8; 32];
        futures::executor::block_on(async {
            c.execute(&mut TestCtx::external(&big),
                &announce_with(&["codex"], &[("cores", 16), ("mem_gb", 64)])).await.unwrap();
            c.execute(&mut TestCtx::external(&small),
                &announce_with(&["codex"], &[("cores", 4), ("mem_gb", 8)])).await.unwrap();
            // tags-only node (direct mode): never matches ANY demand.
            c.execute(&mut TestCtx::external(&bare), &announce_with(&["codex"], &[]))
                .await.unwrap();
            c.commit_block().await.unwrap();
        });

        assert_eq!(capable(&c, "codex", &[("cores", 8)]), vec![big.clone()]);
        // empty demands degrade to plain Providers (all three).
        assert_eq!(capable(&c, "codex", &[]).len(), 3);
        // a dimension nobody announced matches nobody.
        assert!(capable(&c, "codex", &[("gpu", 1)]).is_empty());
        // both dimensions must hold.
        assert_eq!(capable(&c, "codex", &[("cores", 4), ("mem_gb", 32)]), vec![big]);
    }

    #[test]
    fn resources_without_capabilities_reject_and_malformed_resources_reject() {
        let mut c = ungated();
        let me = vec![34u8; 32];
        let mut ctx = TestCtx::external(&me);
        // capacity with nothing to execute is meaningless — reject loudly.
        let err = futures::executor::block_on(
            c.execute(&mut ctx, &announce_with(&[], &[("cores", 8)])),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(_)), "got {err:?}");
        // zero value / bad key reject via validate_resources.
        assert!(futures::executor::block_on(
            c.execute(&mut ctx, &announce_with(&["codex"], &[("cores", 0)]))
        )
        .is_err());
    }

    #[test]
    fn snapshot_round_trip_carries_resources() {
        let mut src = ungated();
        let a = vec![35u8; 32];
        futures::executor::block_on(
            src.execute(&mut TestCtx::external(&a),
                &announce_with(&["codex"], &[("cores", 8)])),
        )
        .unwrap();
        futures::executor::block_on(src.commit_block()).unwrap();
        let bytes = src.snapshot();
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(StateRoot(digest), src.root());

        let mut dst = ungated();
        dst.install(&bytes, src.root()).unwrap();
        assert_eq!(dst.root(), src.root());
        assert_eq!(capable(&dst, "codex", &[("cores", 8)]), vec![a]);
    }
```

Also update every existing test's `announce(&[...])` helper call: keep `announce()` as `announce_with(tags, &[])` so the existing suite compiles unchanged.

- [ ] **Step 2: Run to verify failure**

Run: `ops/build-with.sh cargo test -p capability`
Expected: FAIL — Announce handler ignores resources / new queries unimplemented.

- [ ] **Step 3: Implement.** Mechanical outline (follow the existing style exactly):

1. `struct NodeEntry { tags: BTreeSet<String>, resources: BTreeMap<String, u64> }` (derive `Clone, Debug, PartialEq, Eq`); `announced`/`pending` become `BTreeMap<Vec<u8>, NodeEntry>`; empty means absent keyed on `tags.is_empty()`.
2. `execute` Announce arm: `let tags = Self::validate_tags(capabilities)?; validate_resources(&resources).map_err(Error::Module)?; if tags.is_empty() && !resources.is_empty() { return Err(Error::Module("resources without capabilities (announce at least one tag)".into())); } self.pending.insert(node, NodeEntry { tags, resources });`
3. `effective()`: staged entry with empty tags reads as removal (as today).
4. `snapshot_of`: after the tag list per node, append `resources.len() as u64` LE, then per sorted dimension `push_str(key)` + `value.to_le_bytes()`. A node with zero tags still has no encoding. Zero resources encodes a lone zero count (a tags-only node round-trips).
5. `decode_snapshot`: after tags, read `resource count` (`cur.bound(count, 16, "snapshot resource")` — 8-byte key prefix + 8-byte value), keys strictly increasing utf-8, values u64; reject zero values (no valid encoding — `validate_resources` invariant).
6. `query`: `Providers` filters `entry.tags.contains(&capability)` (unchanged semantics); `Node` returns `entry.tags`; `All` returns tags (reply shapes untouched); add:

```rust
            CapabilityQuery::CapableProviders { capability, demands } => {
                let providers = view
                    .iter()
                    .filter(|(_, e)| e.tags.contains(&capability))
                    .filter(|(_, e)| {
                        demands
                            .iter()
                            .all(|(k, v)| e.resources.get(k).is_some_and(|have| have >= v))
                    })
                    .map(|(key, _)| key.clone())
                    .collect();
                encode_reply(&CapabilityReply::Providers(providers))
            }
            CapabilityQuery::Resources { node } => {
                let resources = view.get(&node).map(|e| e.resources.clone()).unwrap_or_default();
                encode_reply(&CapabilityReply::Resources(resources))
            }
```

7. Module doc header: add one paragraph — resources are announced capacity, the encoding changed (flag day), `CapableProviders` is the assignment filter's read.

- [ ] **Step 4: Run tests**

Run: `ops/build-with.sh cargo test -p capability && ops/build-with.sh cargo clippy -p capability --tests --no-deps`
Expected: PASS, no new lints.

- [ ] **Step 5: Commit**

```bash
git add crates/system/capability/src/lib.rs
git commit -m "feat(capability): registry stores numeric resources; capable-providers filter (FLAG DAY: root encoding v2)"
```

---

### Task 3: saga — demands on the trigger, filtered assignment pool

**Files:**
- Modify: `crates/system/saga/src/interface.rs` (Trigger + stored trigger view)
- Modify: `crates/system/saga/src/lib.rs` (Saga state, snapshot codec, `assignment_pool`)
- Modify (compile fix): every in-repo `SagaMsg::Trigger { ... }` construction site gains `demands: Default::default()` — find them all with `grep -rn 'SagaMsg::Trigger' crates bin --include='*.rs'` (dispatch's site gets real values in Task 4).

**Interfaces:**
- Consumes: `capability::{MAX_RESOURCE_DIMS, validate_resources}` (saga already depends on the capability crate), `CapabilityQuery::CapableProviders`.
- Produces: `SagaMsg::Trigger { ..., demands: BTreeMap<String, u64> }` (`#[serde(default)]`); assignment draws from `CapableProviders` when `capability` is set and demands are non-empty.

- [ ] **Step 1: Write failing tests** (extend saga's existing test module; use its `msg`/`exec`/`commit` helpers and the capability-registry-answering `CaptureCtx` — mirror the existing capability-assignment test, whose name you find via `grep -n 'capability' crates/system/saga/src/lib.rs`):

```rust
    #[test]
    fn demands_filter_the_assignment_pool_via_capable_providers() {
        // ctx answers CapableProviders with only the big node; a trigger
        // carrying demands must assign there, never to the small provider.
        // (extend CaptureCtx's capability-reply stub to key on the decoded
        // query variant: Providers -> both nodes, CapableProviders -> big only.)
        let big = b"node-big".to_vec();
        let mut m = SagaModule::with_assignment("saga", "valset", "capability", LeasePolicy::Strict);
        let mut ctx = capability_ctx_with(vec![b"node-small".to_vec(), big.clone()], vec![big.clone()]);
        exec(&mut m, &mut ctx, &msg(&SagaMsg::Trigger {
            saga_id: "s-demand".into(),
            spec: b"w".to_vec(),
            reply_to: None,
            reply_payload: Vec::new(),
            deadline: Some(100),
            max_attempts: 3,
            lease_views: Some(10),
            capability: Some("codex".into()),
            pinned: None,
            demands: [("cores".to_string(), 8u64)].into_iter().collect(),
        })).unwrap();
        commit(&mut m);
        let pending = assigned_pending(&m, &big);
        assert_eq!(pending.len(), 1, "the demand-capable node holds the lease");
    }

    #[test]
    fn trigger_demands_survive_snapshot_round_trip() {
        // same trigger as above, then snapshot/install into a fresh module:
        // roots equal, and the reassignment pool still filters by demands.
    }

    #[test]
    fn oversized_or_malformed_demands_reject_at_trigger() {
        // 17 dimensions -> Err; a zero value -> Err (validate_resources is THE rule).
    }
```

(Match `Trigger`'s real field list — check `pinned`/field names against `interface.rs:125-155` before writing; the sketch above adds only `demands`.)

- [ ] **Step 2: Run to verify failure**

Run: `ops/build-with.sh cargo test -p saga`
Expected: FAIL — no `demands` field.

- [ ] **Step 3: Implement**

1. `interface.rs` Trigger gains:

```rust
        /// numeric resource demands (e.g. "cores", "mem_gb"). with `capability`
        /// set, assignment draws from providers whose ANNOUNCED capacity covers
        /// every dimension; empty = capability-only assignment (legacy).
        #[serde(default)]
        demands: BTreeMap<String, u64>,
```

2. Trigger validation: `capability::validate_resources(&demands).map_err(Error::Module)?`.
3. `struct Saga` gains `demands: BTreeMap<String, u64>`; snapshot encode after `capability` (count u64 LE, then per sorted key `push_str` + value LE); decode mirrors with strictly-increasing keys and non-zero values; root moves (flag day, already declared).
4. `assignment_pool(ctx, capability, demands)`: when `capability` is `Some(tag)` and `!demands.is_empty()`, query `CapabilityQuery::CapableProviders { capability: tag, demands }` instead of `Providers`; reply decoding is identical (`CapabilityReply::Providers`). Untagged sagas ignore demands (valset assignment as today).
5. Fix every in-repo Trigger construction with `demands: Default::default()`.

- [ ] **Step 4: Run tests**

Run: `ops/build-with.sh cargo test -p saga && ops/build-with.sh cargo clippy -p saga --tests --no-deps && ops/build-with.sh cargo check --workspace`
Expected: PASS; whole workspace still compiles.

- [ ] **Step 5: Commit**

```bash
git add -A crates/system/saga crates bin
git commit -m "feat(saga): demand-filtered capability assignment (FLAG DAY: saga snapshot v+1)"
```

---

### Task 4: dispatch — demands ride Dispatch → Trigger + WorkSpec

**Files:**
- Modify: `crates/system/dispatch/src/interface.rs` (`WorkSpec`, `DispatchMsg::Dispatch`)
- Modify: `crates/system/dispatch/src/lib.rs` (Dispatch handler threads demands into the Trigger AND the WorkSpec)
- Modify (compile fix): `WorkSpec { ... }` construction sites in `crates/system/dispatch-oracle/src/lib.rs` tests and anywhere else `grep -rn 'WorkSpec {' crates bin` finds — add `demands: Default::default()`.

**Interfaces:**
- Consumes: Task 3's Trigger `demands`.
- Produces: `WorkSpec { ..., demands: BTreeMap<String, u64> }` (`#[serde(default)]`), `DispatchMsg::Dispatch { dispatch_id, recipe_id, payload, demands: BTreeMap<String, u64> }` (`#[serde(default)]`). The host reads demands from the WorkSpec — saga stays spec-opaque; dispatch composes both from the one source.

- [ ] **Step 1: Write failing test** (dispatch's test module; mirror how the existing dispatch-flow test drives `Dispatch` and captures the emitted saga Trigger):

```rust
    #[test]
    fn dispatch_demands_reach_both_the_trigger_and_the_work_spec() {
        // register a recipe, execute DispatchMsg::Dispatch { demands: {"cores": 4} },
        // then decode the captured SagaMsg::Trigger: trigger.demands == {"cores": 4}
        // AND decode_work_spec(trigger.spec).demands == {"cores": 4}.
    }

    #[test]
    fn work_spec_without_demands_field_still_decodes() {
        let old = br#"{"kind":"dispatch-work-v1","dispatch_id":"d","capability":"c","payload":[]}"#;
        assert!(decode_work_spec(old).unwrap().demands.is_empty());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `ops/build-with.sh cargo test -p dispatch`
Expected: FAIL — no `demands` field.

- [ ] **Step 3: Implement.** Add both fields with `#[serde(default)]` and doc comments ("numeric resource demands, validated by `capability::validate_resources` at dispatch time; empty = demandless legacy job"). In the Dispatch handler validate, then thread `demands` into the composed `WorkSpec` and the emitted `SagaMsg::Trigger` (same value, one source). Fix `WorkSpec` construction sites across the workspace with `demands: Default::default()`.

- [ ] **Step 4: Run tests**

Run: `ops/build-with.sh cargo test -p dispatch -p dispatch-oracle && ops/build-with.sh cargo clippy -p dispatch --tests --no-deps && ops/build-with.sh cargo check --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A crates
git commit -m "feat(dispatch): demands ride Dispatch -> saga trigger + work spec"
```

---

### Task 5: runs — RequestRun passes demands through

**Files:**
- Modify: `crates/apps/runs/src/interface.rs` (`RunsMsg::RequestRun`)
- Modify: `crates/apps/runs/src/dispatch_flow.rs` / the RequestRun handler (find with `grep -n 'RequestRun' crates/apps/runs/src/*.rs`) — thread demands into `DispatchMsg::Dispatch`
- Test: `crates/apps/runs/src/tests/` (the module that already exercises RequestRun; find with `grep -rn 'RequestRun' crates/apps/runs/src/tests/`)

**Interfaces:**
- Consumes: Task 4's `DispatchMsg::Dispatch.demands`.
- Produces: `RunsMsg::RequestRun { agent_id, channel_id, anchor_seq, demands: BTreeMap<String, u64> }` (`#[serde(default)]`). Chat-mention / page-comment / jobs intakes stay demandless (empty map) — the explicit-run path is the only demand surface in Phase 1.

- [ ] **Step 1: Write failing test** — in the runs test module, submit a `RequestRun` with `demands: {"cores": 4}` and assert the captured `DispatchMsg::Dispatch` carries the same map (mirror the existing RequestRun test's capture pattern).

- [ ] **Step 2: Run to verify failure**

Run: `ops/build-with.sh cargo test -p runs`
Expected: FAIL — no `demands` field.

- [ ] **Step 3: Implement.** Add the serde-defaulted field; pass it verbatim to the Dispatch emit in the RequestRun path; all other `DispatchMsg::Dispatch` emits in runs use `Default::default()`.

- [ ] **Step 4: Run tests**

Run: `ops/build-with.sh cargo test -p runs && ops/build-with.sh cargo clippy -p runs --tests --no-deps`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A crates/apps/runs
git commit -m "feat(runs): explicit runs carry per-run resource demands"
```

---

### Task 6: dispatch-oracle — resource ledger + gate admission

**Files:**
- Create: `crates/system/dispatch-oracle/src/ledger.rs`
- Modify: `crates/system/dispatch-oracle/src/lib.rs` (`gate`, `ExecJob`)
- Modify: `crates/system/dispatch-oracle/src/pool.rs` (ledger wiring, reserve/release around execution)

**Interfaces:**
- Consumes: `WorkSpec.demands` (Task 4).
- Produces: `pub struct ResourceLedger` with `pub fn new(capacity: BTreeMap<String, u64>) -> Self`, `fn fits(&self, demands) -> bool`, `fn reserve(&self, key: &str, demands) -> ReservationGuard` (RAII release), `fn release(&self, key: &str)`. `gate()` gains a `ledger: &ResourceLedger` param; `ExecJob` gains `pub demands: BTreeMap<String, u64>`. `DispatchPool::with_limit` gains a `capacity: BTreeMap<String, u64>` param (empty = direct node: demandless jobs only). Re-export `ResourceLedger` from lib.rs.

- [ ] **Step 1: Write failing tests** (in `ledger.rs` + extend lib.rs gate tests):

```rust
// ledger.rs tests
#[test]
fn fits_is_per_dimension_and_absent_is_not_infinite() {
    let l = ResourceLedger::new(res(&[("cores", 8), ("mem_gb", 16)]));
    assert!(l.fits(&res(&[("cores", 8)])));
    assert!(!l.fits(&res(&[("cores", 9)])));
    assert!(!l.fits(&res(&[("gpu", 1)])), "absent dimension never fits");
    assert!(l.fits(&res(&[])), "demandless always fits");
    // empty capacity (direct node): only demandless fits.
    let bare = ResourceLedger::new(Default::default());
    assert!(bare.fits(&res(&[])));
    assert!(!bare.fits(&res(&[("cores", 1)])));
}

#[test]
fn reservations_subtract_and_release_on_drop() {
    let l = ResourceLedger::new(res(&[("cores", 8)]));
    let guard = l.reserve("s1:0", &res(&[("cores", 6)]));
    assert!(!l.fits(&res(&[("cores", 4)])), "6 of 8 reserved");
    assert!(l.fits(&res(&[("cores", 2)])));
    drop(guard);
    assert!(l.fits(&res(&[("cores", 8)])), "released on drop");
}
```

```rust
// lib.rs gate tests (reuse mock_specs_only + StubProvider patterns)
#[test]
fn an_own_lease_over_capacity_is_skipped_so_the_lease_can_rotate() {
    // providers resolve, but ledger capacity {"cores": 4} < demands {"cores": 8}
    // -> Gated::Skip (NOT an inline error: the saga lease expires and the next
    // attempt rendezvouses elsewhere; an error result would consume the attempt
    // with a lie — this node was never able to run it).
}

#[test]
fn an_announcement_over_capacity_is_not_claimed() {
    // unassigned request + over-capacity demands -> Gated::Skip, never Accept.
}

#[test]
fn demandless_jobs_bypass_capacity_even_on_a_bare_ledger() {
    // empty demands + empty capacity -> Gated::Execute as today.
}
```

- [ ] **Step 2: Run to verify failure**

Run: `ops/build-with.sh cargo test -p dispatch-oracle`
Expected: FAIL — ledger module missing / gate signature.

- [ ] **Step 3: Implement**

```rust
//! ledger.rs — the host-local admission ledger: announced capacity minus the
//! demands of currently RUNNING jobs. deliberately process-local (consensus
//! never sees load); a crashed node's over-commitments die with its leases.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub struct ResourceLedger {
    capacity: BTreeMap<String, u64>,
    running: Arc<Mutex<BTreeMap<String, BTreeMap<String, u64>>>>,
}

impl ResourceLedger {
    pub fn new(capacity: BTreeMap<String, u64>) -> Self {
        Self { capacity, running: Arc::new(Mutex::new(BTreeMap::new())) }
    }

    /// free = capacity − Σ running, per dimension; a demanded dimension the
    /// capacity never named is a mismatch (absent ≠ infinite). empty demands
    /// trivially fit — the demandless legacy path costs nothing here.
    pub fn fits(&self, demands: &BTreeMap<String, u64>) -> bool {
        let running = self.running.lock().expect("ledger lock");
        demands.iter().all(|(dim, want)| {
            let Some(cap) = self.capacity.get(dim) else { return false };
            let used: u64 = running.values().filter_map(|d| d.get(dim)).sum();
            cap.saturating_sub(used) >= *want
        })
    }

    /// record a run's demands under its attempt key; the guard releases on
    /// drop, so every exit path (ok, error, panic-unwind) frees the slot.
    pub fn reserve(&self, key: &str, demands: &BTreeMap<String, u64>) -> ReservationGuard {
        if !demands.is_empty() {
            self.running.lock().expect("ledger lock")
                .insert(key.to_string(), demands.clone());
        }
        ReservationGuard { running: Arc::clone(&self.running), key: key.to_string() }
    }
}

pub struct ReservationGuard {
    running: Arc<Mutex<BTreeMap<String, BTreeMap<String, u64>>>>,
    key: String,
}

impl Drop for ReservationGuard {
    fn drop(&mut self) {
        self.running.lock().expect("ledger lock").remove(&self.key);
    }
}
```

`gate()` changes (both the own-lease and announcement arms, BEFORE resolve so the reason is capacity, after payload-shape checks):

```rust
// own lease: over capacity -> Skip. deliberately NOT an inline error — the
// lease expires and the next attempt rendezvouses to another provider;
// ponytail: costs one lease window; a Decline op for instant rotation is the
// upgrade path if that window hurts.
if !ledger.fits(&work.demands) {
    return Gated::Skip;
}
// announcement: same check before accept_op — never claim what cannot fit.
```

`ExecJob` gains `demands`; `pool.rs::spawn_exec` takes `let _reservation = self.ledger.reserve(&key_string, &job.demands);` inside the spawned task (before the semaphore acquire — capacity was already promised at gate time), held to end of task. `DispatchPool::with_limit` gains `capacity: BTreeMap<String, u64>`; `new()` passes `Default::default()` until Task 8 wires real capacity. Update `oracle_pool::build` call sites with `Default::default()` (compile fix; real values Task 8).

- [ ] **Step 4: Run tests**

Run: `ops/build-with.sh cargo test -p dispatch-oracle && ops/build-with.sh cargo clippy -p dispatch-oracle --tests --no-deps && ops/build-with.sh cargo check --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A crates/system/dispatch-oracle bin
git commit -m "feat(dispatch-oracle): host-local resource ledger gates lease and accept"
```

---

### Task 7: capability-host — RunContext limits + podman sandbox backend

**Files:**
- Create: `crates/system/capability-host/src/sandbox.rs`
- Modify: `crates/system/capability-host/src/lib.rs` (`RunContext`, `CliProvider::command` at :329, `discover` at :939 threading the backend)
- Modify: `crates/system/capability-host/src/spec.rs` (optional `[sandbox] rw_dirs = ["~/.claude"]` per-executor auth-state dirs)

**Interfaces:**
- Consumes: `ExecJob.demands` → pool passes into `RunContext`.
- Produces: `pub enum SandboxBackend { Direct, Podman { image: String } }` (Default = Direct); `RunContext { ..., pub limits: BTreeMap<String, u64> }` (Default empty); `CapabilitySpec { ..., pub rw_dirs: Vec<String> }` (home-relative, default empty); `pub fn wrap_podman(image, bin, args, workdir, envs, ro_paths, rw_dirs, limits) -> (PathBuf, Vec<String>)` — pure argv translation, unit-testable without podman. `discover()` gains a `backend: SandboxBackend` param.

- [ ] **Step 1: Write failing tests** (in `sandbox.rs` — pure argv assembly, no podman needed):

```rust
#[test]
fn podman_wrap_translates_limits_mounts_and_env() {
    let (bin, argv) = wrap_podman(
        "docker.io/library/node:22-slim",
        Path::new("/usr/bin/claude"),
        &["--print".into()],
        Path::new("/tmp/work"),
        &[("FOO".into(), "bar".into())],
        &[PathBuf::from("/opt/skills")],
        &[PathBuf::from("/home/u/.claude")],
        &[("cores".into(), 4u64), ("mem_gb".into(), 8u64)].into_iter().collect(),
    );
    assert_eq!(bin, PathBuf::from("podman"));
    let s = argv.join(" ");
    assert!(s.starts_with("run --rm --network=host"));
    assert!(s.contains("--cpus 4") && s.contains("--memory 8g"));
    assert!(s.contains("-v /tmp/work:/tmp/work") && s.contains("-w /tmp/work"));
    assert!(s.contains("-v /usr/bin/claude:/usr/bin/claude:ro"));
    assert!(s.contains("-v /opt/skills:/opt/skills:ro"));
    assert!(s.contains("-v /home/u/.claude:/home/u/.claude"), "auth state rw");
    assert!(s.contains("-e FOO=bar"));
    assert!(s.ends_with("docker.io/library/node:22-slim /usr/bin/claude --print"));
}

#[test]
fn dimensions_without_a_podman_flag_are_ignored_not_errors() {
    // {"gpu": 1} produces no flag — scheduling already matched it; the
    // backend enforces only what it knows how to enforce.
}
```

And a spec test in `spec.rs`: `[sandbox] rw_dirs = ["~/.claude"]` parses; an absolute or `..`-containing entry rejects loudly.

- [ ] **Step 2: Run to verify failure**

Run: `ops/build-with.sh cargo test -p capability-host`
Expected: FAIL — module missing.

- [ ] **Step 3: Implement**

`sandbox.rs` — paths mounted at IDENTICAL container paths (no path translation anywhere else in the codebase):

```rust
//! the sandbox backend seam: how a provider child is spawned. Direct is the
//! historical spawn; Podman wraps the identical argv in a rootless container
//! that enforces the run's numeric limits. paths are mounted at identical
//! container paths so workdir/session/skill logic upstream stays path-blind.
//! HOME is NOT mounted: only the spec's [sandbox] rw_dirs (CLI auth/state)
//! cross the boundary, so the node's data dir and user key stay outside —
//! the D7 enforcement mechanism the provider doc deferred.
//! ponytail: --network=host keeps loopback MCP reachable; a private netns
//! with a gateway route is the upgrade path if network isolation matters.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SandboxBackend {
    #[default]
    Direct,
    Podman { image: String },
}

pub fn wrap_podman(
    image: &str,
    bin: &Path,
    args: &[String],
    workdir: &Path,
    envs: &[(String, String)],
    ro_paths: &[PathBuf],
    rw_dirs: &[PathBuf],
    limits: &BTreeMap<String, u64>,
) -> (PathBuf, Vec<String>) {
    let mut argv: Vec<String> = vec!["run".into(), "--rm".into(), "--network=host".into(), "-i".into()];
    if let Some(cores) = limits.get("cores") {
        argv.extend(["--cpus".into(), cores.to_string()]);
    }
    if let Some(mem) = limits.get("mem_gb") {
        argv.extend(["--memory".into(), format!("{mem}g")]);
    }
    argv.extend(["-v".into(), format!("{d}:{d}", d = workdir.display())]);
    argv.extend(["-w".into(), workdir.display().to_string()]);
    argv.extend(["-v".into(), format!("{b}:{b}:ro", b = bin.display())]);
    for p in ro_paths {
        argv.extend(["-v".into(), format!("{p}:{p}:ro", p = p.display())]);
    }
    for d in rw_dirs {
        argv.extend(["-v".into(), format!("{d}:{d}", d = d.display())]);
    }
    for (k, v) in envs {
        argv.extend(["-e".into(), format!("{k}={v}")]);
    }
    argv.push(image.to_string());
    argv.push(bin.display().to_string());
    argv.extend(args.iter().cloned());
    (PathBuf::from("podman"), argv)
}
```

`CliProvider::command()` — at the top, when the provider's backend is `Podman` AND `ctx.limits` is non-empty OR always-when-podman (decide: ALWAYS when podman — a sandboxed node sandboxes everything it runs, demandless included, limits flags only when present): build `(bin, argv)` via `wrap_podman` with `envs = ctx.env`, `ro_paths = ctx.path_entries` dirs + spec bin, `rw_dirs` = spec's `rw_dirs` expanded against the real `$HOME`; construct `tokio::process::Command::new(bin)` with that argv, keep the existing stdio/kill_on_drop/current_dir handling (cwd stays the host workdir — harmless under podman, meaningful under direct). PATH entries still exported via `-e PATH=...` including container-side identical paths.

`discover()`/`discover_with_sink()` gain `backend: SandboxBackend`, stored on each `CliProvider`. Compile-fix call sites (`bin/node/src/validator/run.rs:269`, `bin/node/src/replica/park.rs:478`, `bin/noded/src/oracle_pool.rs:67`) with `SandboxBackend::Direct` (real wiring Task 8).

`RunContext` gains `pub limits: BTreeMap<String, u64>` (Default empty); `pool.rs` fills it from `ExecJob.demands`.

Update the two builtin specs (`specs/*.toml`) with their `[sandbox] rw_dirs`: claude → `["~/.claude", "~/.claude.json"]`-style per what the CLI actually persists (verify on box), codex → `["~/.codex"]`.

- [ ] **Step 4: Run tests + live smoke (podman present on the dev box)**

Run: `ops/build-with.sh cargo test -p capability-host && ops/build-with.sh cargo clippy -p capability-host --tests --no-deps`
Expected: PASS.
Live check (manual, not CI): `podman run --rm --cpus 1 --memory 1g docker.io/library/node:22-slim node -e 'console.log(1)'` — verifies rootless podman + cpu delegation on this host. If `--cpus` errors with a cgroup delegation message, record it in the PR as the known Debian caveat; scheduling still works, memory limits still apply.

- [ ] **Step 5: Commit**

```bash
git add -A crates/system/capability-host bin
git commit -m "feat(capability-host): podman sandbox backend enforces per-run limits"
```

---

### Task 8: bin/node + bin/noded — opt-in default, sandbox config, resource probe, announcer

**Files:**
- Modify: `bin/node/src/config/node_toml.rs` (`sandbox`, `sandbox_image`, `sandbox_cores`, `sandbox_mem_gb` fields)
- Modify: `bin/node/src/config/resolve.rs:243,532` (`announce_capabilities.unwrap_or(true)` → `unwrap_or(false)`; resolve sandbox knobs; update the default test at :752)
- Create: `bin/node/src/host_resources.rs` (probe)
- Modify: `bin/node/src/validator/announce.rs` (`CapabilityAnnouncer` carries + compares resources)
- Modify: `bin/node/src/validator/run.rs`, `bin/node/src/replica/park.rs`, `bin/node/src/oracle_pool.rs`, `bin/noded/src/oracle_pool.rs` (thread backend + capacity)

**Interfaces:**
- Consumes: `SandboxBackend`, `ResourceLedger` capacity param, `CapabilityQuery::Resources`.
- Produces: node.toml `sandbox = "direct" | "podman"` (absent = direct), `sandbox_image` (default `docker.io/library/node:22-slim`), `sandbox_cores`/`sandbox_mem_gb` overrides; `host_resources::probe() -> BTreeMap<String, u64>`; announcer announces `{tags, resources}` where resources are non-empty iff backend is sandboxed.

- [ ] **Step 1: Write failing tests**

`host_resources.rs`:

```rust
#[test]
fn probe_reports_nonzero_cores_and_mem_on_this_host() {
    let r = probe();
    assert!(r.get("cores").copied().unwrap_or(0) >= 1);
    assert!(r.get("mem_gb").copied().unwrap_or(0) >= 1);
}
```

`resolve.rs`: rewrite `announce_capabilities_defaults_on_and_parses_off` → `announce_capabilities_defaults_OFF_and_parses_on` (assert absent ⇒ `false`, `announce_capabilities = true` parses on); add `sandbox_parses_and_defaults_direct` (absent ⇒ Direct; `sandbox = "podman"` ⇒ Podman with the default image; unknown value ⇒ loud resolve error).

`announce.rs` decision-core tests: `decide` fires when committed TAGS match but committed RESOURCES differ; stays quiet when both match; a direct-backend announcer always carries empty resources.

- [ ] **Step 2: Run to verify failure**

Run: `ops/build-with.sh cargo test -p node`
Expected: FAIL.

- [ ] **Step 3: Implement**

`host_resources.rs` (Linux `/proc/meminfo` MemTotal, macOS `sysctl -n hw.memsize`, cores via `std::thread::available_parallelism`; overrides win):

```rust
//! probed host capacity for the capability announce — total machine
//! resources, deliberately NOT free memory: capacity is a standing promise,
//! the ledger handles moment-to-moment load.

pub(crate) fn probe() -> BTreeMap<String, u64> {
    let mut r = BTreeMap::new();
    if let Ok(n) = std::thread::available_parallelism() {
        r.insert("cores".into(), n.get() as u64);
    }
    if let Some(gb) = total_mem_gb() {
        r.insert("mem_gb".into(), gb);
    }
    r
}

#[cfg(target_os = "linux")]
fn total_mem_gb() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kb: u64 = text.lines().find(|l| l.starts_with("MemTotal:"))?
        .split_whitespace().nth(1)?.parse().ok()?;
    Some((kb / (1024 * 1024)).max(1))
}

#[cfg(target_os = "macos")]
fn total_mem_gb() -> Option<u64> { /* sysctlbyname hw.memsize via libc */ }
```

Threading (all four call sites, same pattern):
1. resolve → `BootEnv` carries `sandbox: SandboxBackend` + `sandbox_capacity: BTreeMap<String, u64>` (probe() with per-key overrides applied, EMPTY when backend is Direct).
2. `capability_host::discover(agent_dirs, sink, backend.clone())`.
3. `oracle_pool::build(..., capacity.clone())` → `DispatchPool::with_limit(..., capacity)`.
4. `CapabilityAnnouncer::new(me, capabilities, resources)` where `resources = capacity` (already empty for Direct); `announce_capabilities == false` still empties the TAG set exactly as today (registry removal semantics unchanged). `maybe_announce` reads BOTH `CapabilityQuery::Node` and `CapabilityQuery::Resources` for the committed view; `decide(committed_tags, committed_resources)` re-announces when either differs. `ResidentAnnouncer` passes through identically.

- [ ] **Step 4: Run tests + gates**

Run: `ops/build-with.sh cargo test -p node && ops/build-with.sh cargo clippy -p node --tests --no-deps && ops/build-with.sh cargo check -p noded && ops/build-with.sh cargo check --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A bin
git commit -m "feat(node): opt-in capability serving (default OFF), sandbox config + capacity announce"
```

---

### Task 9: app (TS) — demands on the explicit-run surface

**Files:**
- Modify: `app/src/domain/runs-client.ts:127` (`request_run` payload gains optional `demands`)
- Modify: the component invoking `request_run` (find with `grep -rn 'request_run' app/src --include='*.tsx' --include='*.ts'`) — optional "Resources (cores / memory GB)" inputs, omitted entirely when blank
- Test: `app/src/domain/runs-client.test.ts`

**Interfaces:**
- Consumes: `RunsMsg::RequestRun.demands` (serde-defaulted, so omitting the key is valid legacy wire).
- Produces: `requestRun(..., demands?: Record<string, number>)`; UI sends `{ cores, mem_gb }` only when the operator filled the fields.

- [ ] **Step 1: Write failing test** — runs-client test asserting the composed wire JSON includes `"demands":{"cores":4,"mem_gb":8}` when passed, and omits the key entirely when not.

- [ ] **Step 2: Run to verify failure**

Run: `cd app && bun test src/domain/runs-client.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement.** Optional param, spread into the message only when defined; two small numeric inputs on the explicit-run form (follow the form's existing field styling; no new components).

- [ ] **Step 4: Run tests**

Run: `cd app && bun test`
Expected: PASS (full suite — catches wire-shape regressions elsewhere).

- [ ] **Step 5: Commit**

```bash
git add app/src
git commit -m "feat(app): per-run resource demands on the explicit run form"
```

---

### Task 10: end-to-end verification + PR

**Files:** none new — verification and delivery.

- [ ] **Step 1: Full gate sweep** (touch a `.rs` in each touched crate first — cached clippy is vacuous):

```bash
for c in capability saga dispatch dispatch-oracle capability-host runs; do
  ops/build-with.sh cargo clippy -p $c --tests --no-deps || exit 1
  ops/build-with.sh cargo test -p $c || exit 1
done
ops/build-with.sh cargo clippy -p node --tests --no-deps
ops/build-with.sh cargo test -p node
ops/build-with.sh cargo check -p files --no-default-features
cd app && bun test
```

Expected: all green (the pre-existing `duckfs_engine_round_trips_across_two_nodes` flake is known-red on dev; anything else red is yours).

- [ ] **Step 2: Live smoke on the dev box** — single-node dev net: opt-in via node.toml (`announce_capabilities = true`, `sandbox = "podman"`), boot, verify the announce lands (query `capability` `All` + `Resources` over RPC), submit one explicit run with `demands: {"cores": 1}` and one with demands exceeding capacity; the first executes (verify the child ran under podman via the run output), the second dies by saga deadline with no assignment. Record both observations in the PR body.

- [ ] **Step 3: PR against dev**

```bash
git push -u origin compute-capability-p1
gh pr create --base dev --title "compute-aware capability scheduling + podman sandbox (phase 1)" --body "<summary; FLAG DAY: capability+saga roots; BEHAVIOR CHANGE: announce default OFF; spec + verification notes>"
```

PR body must call out: the two flag-day encodings, the default-OFF migration (existing nodes drop from the registry until opted in), the podman cpu-delegation caveat if observed, and Phase 2/3 scope explicitly deferred (tart, preflight UI, agent onboarding).
