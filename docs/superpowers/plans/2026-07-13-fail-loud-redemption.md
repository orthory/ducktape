# Fail-Loud Invite Redemption Implementation Plan (PR1)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A joiner presenting a consumed (or otherwise unredeemable) invite terminates loudly with a reason — never waits forever.

**Architecture:** Three thin changes on the existing lobby plane: (1) a regression e2e that pins the reuse behavior end to end, (2) strict `JoinReply` decoding + a fatal-audit of member-side rejects, (3) a hard deadline on the joiner's lobby-announce phase as the universal backstop. No new types, no new files except the test.

**Tech Stack:** Rust (bin/node, crates/system/governance), existing `NetworkShapeCluster` e2e harness.

**Spec:** `docs/superpowers/specs/2026-07-13-coordinator-invites-thin-client-design.md` (Design 1).

## Global Constraints

- Branch from `origin/dev`, work in a worktree under `<primary-checkout>/.worktree/fail-loud-redemption`, PR against `dev` (repo rule).
- Build/lint through `ops/build-with.sh`: `ops/build-with.sh cargo clippy -p <crate> --tests --no-deps` per touched crate; never `cargo fmt --all` (format only touched code).
- NO backward compatibility (user mandate): strip serde-default wire shims in touched code; no version negotiation. Old member binaries become unsupported.
- e2e tests spawn real nodes: run serially (`serial()` guard in harness), generous timeouts, and keep Cargo target inside the worktree (never /tmp).
- Test-lane doctrine (user mandate): node semantics → simnode, UI → fleet. This plan's scenario needs the REAL join plane — process `exit(1)`, lobby over sockets, daemon-log FATAL markers — none of which simnode has (it registers noded's module set: no governance/valset, no lobby). The `NetworkShapeCluster` e2e is the only lane that can observe the contract under test; governance-level single-use semantics stay covered by the module rig (`crates/system/governance/tests/invite_redemption.rs`).
- Investigation result already settled during planning: invite **expiry** is enforced at joiner-side decode (`bin/node/src/config/invite.rs:614`, "this invite has expired"). The remaining hole (a hand-crafted announce reusing an expired blob's token, which never expires) is closed by PR3, which moves expiry into the token. **No expiry change in this PR.**
- UI: `app/src-tauri/src/workspaces/phase.rs::classify` already maps a `FATAL` daemon.log line to the `"fatal"` phase and `JoinProgress.tsx` renders it. **No app change in this PR** — the joiner's FATAL line is the contract.

---

### Task 1: Regression e2e — a reused invite kills the second joiner loudly

> **DRIFT RESOLUTION (2026-07-13, controller):** the scenario already exists —
> `bin/node/tests/join_request_e2e.rs::a_spent_invite_is_refused_loudly_on_both_ends`
> (added with the loud-spent-invite fix, commit c277a382) pins the member refusal
> and the joiner FATAL line via kill+wipe+rejoin on idx 1 (the harness is
> hard-wired to 2 nodes; there is no idx 2, and network-shape founders never
> print "converged app_hash="). Task 1 therefore became: strengthen THAT test in
> place with the two missing assertions — `fresh invite` actionable guidance and
> `wait_exit` (process actually died) — plus a mirrored
> `NetworkShapeCluster::wait_exit`. The pseudocode below is superseded; kept for
> the four-assertion contract it defines.

The user observed a joiner "just waiting" on a consumed invite. Current code *should* already fail loudly (`ingress.rs:200-230` sends `fatal:true`; `wiring.rs:382-395` exits 1). This test pins that end to end with current binaries; if it FAILS, we found the real bug — fix it here before proceeding.

**Files:**
- Create: `bin/node/tests/invite_reuse_e2e.rs`
- Read for harness reference: `bin/node/tests/common/mod.rs` (`NetworkShapeCluster`: `new` :139, `init_founder` :156, `invite` :191, `join_friend` :237, `spawn` :271, `kill` :297, `wait_marker` :399), `bin/node/tests/live_admission_e2e.rs` (usage examples)

**Interfaces:**
- Consumes: `NetworkShapeCluster` as-is. If it lacks an "assert child exited" helper, add `pub fn wait_exit(&mut self, idx: usize, timeout: Duration)` to it, copying `Cluster::wait_exit` (`common/mod.rs:669`) verbatim.
- Produces: the log markers this test asserts (`invite already redeemed`, `FATAL`) become load-bearing; Tasks 2–4 must not change their wording without updating this test.

- [ ] **Step 1: Write the test**

```rust
//! a SPENT invite must refuse the second joiner LOUDLY: the member replies
//! fatal, the joiner prints FATAL and exits — it must never retry forever.

mod common;

use std::time::Duration;

use common::{NetworkShapeCluster, poll_until, serial};

const CONVERGE: Duration = Duration::from_secs(180);

#[test]
fn a_spent_invite_refuses_the_second_joiner_loudly() {
    let _serial = serial();
    let mut cluster = NetworkShapeCluster::new();
    cluster.init_founder("reuse-net");
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));
    cluster.wait_marker(0, "converged app_hash=", CONVERGE);

    // ONE invite, redeemed by friend A (auto-redemption over the lobby).
    let invite = cluster.invite();
    cluster.join_friend(&invite); // A's workspace (idx 1)
    cluster.spawn(1);
    cluster.wait_marker(1, "joiner mode:", Duration::from_secs(60));
    // the founder auto-submits Redeem on A's announce; wait for it to land.
    cluster.wait_marker(0, "invite redemption submitted", CONVERGE);
    poll_until("A's redemption to commit", CONVERGE, || {
        redemption_count(&cluster, 0).filter(|n| *n >= 1)
    });

    // friend B presents the SAME blob: the member must refuse permanently
    // and B must stop LOUDLY instead of spinning.
    cluster.join_friend(&invite); // B's workspace (idx 2)
    cluster.spawn(2);
    let fatal = cluster.wait_marker(2, "FATAL", Duration::from_secs(120));
    assert!(fatal.contains("invite already redeemed"), "names the reason: {fatal}");
    assert!(fatal.contains("fresh invite"), "actionable guidance: {fatal}");
    // and the process actually died (no silent retry loop).
    cluster.wait_exit(2, Duration::from_secs(30));
}

/// committed redemptions visible on node `idx`, via the governance query.
fn redemption_count(cluster: &NetworkShapeCluster, idx: usize) -> Option<usize> {
    use governance::{GovQuery, GovReply, decode_reply, encode_query};
    let reply = cluster.query(idx, "governance", &encode_query(&GovQuery::Redemptions))?;
    match decode_reply(&reply) {
        Ok(GovReply::Redemptions(views)) => Some(views.len()),
        _ => None,
    }
}
```

Adapt mechanical details to the harness as found (e.g. `marker()`/`poll_until` signatures, whether `join_friend` spawns or only materializes the workspace — mirror how `live_admission_e2e.rs` drives it). The four assertions are the contract: member refuses, joiner prints FATAL naming the reason, guidance is actionable, process exits.

- [ ] **Step 2: Run it**

Run: `ops/build-with.sh cargo test -p ducktape-node --test invite_reuse_e2e -- --nocapture`
Expected: PASS if the current fail-loud path works. **If it FAILS (joiner keeps retrying): STOP — that is the user's bug reproduced.** Diagnose which leg broke (member never sent `fatal:true`? joiner never decoded it? lobby never reached the member?) using the two daemon logs, fix the broken leg, and record the root cause in the PR description. Then re-run to green.

- [ ] **Step 3: Commit**

```bash
git add bin/node/tests/join_request_e2e.rs bin/node/tests/common/mod.rs
git commit -m "test(node): pin fail-loud refusal of a spent invite end to end"
```

---

### Task 2: Strict JoinReply decode — drop the serde back-compat shims

`JoinReply.fatal` and `JoinReply.cap` carry `#[serde(default)]` so an OLD member's reply decodes as non-fatal — exactly the silent-forever-retry hole. No-backcompat mandate: strip both; a reply missing fields fails decode, is skipped by the reply printer, and Task 4's deadline catches the stall.

**Files:**
- Modify: `bin/node/src/lobby.rs:45-57` (the `JoinReply` variant), tests at `lobby.rs:327-389`

**Interfaces:**
- Consumes: nothing new.
- Produces: `LobbyMsg::JoinReply { recorded: bool, detail: String, cap: Option<Vec<u8>>, fatal: bool }` — same shape, strict decode. Members and joiners must both be current binaries (flag-day note for the PR).

- [ ] **Step 1: Update the shim test to assert strict decode**

Replace `a_reply_missing_the_cap_field_defaults_to_none` (`lobby.rs:375-389`) with:

```rust
#[test]
fn a_reply_missing_fields_is_rejected_not_defaulted() {
    // pre-cutover members omit `cap`/`fatal`. There is NO compat decode:
    // such a reply is skipped (undecodable) and the joiner's lobby-phase
    // deadline handles the stall. In-place wire updates, no version tags.
    let old_wire = br#"{"join_reply":{"recorded":true,"detail":"awaiting approval"}}"#;
    assert!(decode_msg(old_wire).is_err());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `ops/build-with.sh cargo test -p ducktape-node --lib lobby -- --nocapture` (adjust test path if lobby tests are integration-style: `cargo test -p ducktape-node lobby::`)
Expected: FAIL — the old wire still decodes because the defaults are still present.

- [ ] **Step 3: Remove the `#[serde(default)]` attributes**

In `lobby.rs`, delete the two `#[serde(default)]` lines on `cap` and `fatal` (and prune the now-stale doc-comment sentences about wire back-compat on both fields).

- [ ] **Step 4: Run the lobby tests**

Run: `ops/build-with.sh cargo test -p ducktape-node lobby`
Expected: PASS (roundtrip tests already construct full replies).

- [ ] **Step 5: Commit**

```bash
git add bin/node/src/lobby.rs
git commit -m "fix(node): decode JoinReply strictly — no serde-default wire shims"
```

---

### Task 3: Member reject audit — every permanent reject replies fatal

`on_lobby` (`bin/node/src/validator/run/ingress.rs:128-330`) has five reject paths. Two are PERMANENT for this blob but reply non-fatal today. Fix the mapping; leave transient/standing-adjacent replies non-fatal.

| Reject | Today | Correct | Why |
|---|---|---|---|
| crypto verify fails (`ingress.rs:164-170`) | non-fatal | **fatal** | tampered/foreign blob never verifies later |
| already a validator (`:178-181`) | non-fatal | non-fatal | joiner HAS standing — success-adjacent window |
| already a resident (`:182-190`) | non-fatal | non-fatal | same |
| issuer no longer a member (`:191-199`) | non-fatal | **fatal** | a removed member's invites are dead forever |
| spent nonce (`:200-230`) | fatal | fatal | already correct |
| redeem submit failed (`:326-329`) | non-fatal | non-fatal | transient node-side error |

**Files:**
- Modify: `bin/node/src/validator/run/ingress.rs:164-199`

**Interfaces:**
- Consumes: `send_reply(recorded, detail, cap, fatal)` closure already in scope (`ingress.rs:145`).
- Produces: fatal replies for the two flipped paths; PR3 relies on "crypto fail ⇒ fatal" to cover target-mismatch for free.

- [ ] **Step 1: Flip the two permanent rejects to fatal**

```rust
// crypto first (pure, cheap): the token must verify for THIS network and
// the announced key must prove itself. a verify failure is PERMANENT for
// this blob (tampered, foreign, or malformed) — fail it loudly.
let verified = match lobby::verify_join_request(&msg, namespace) {
    Ok(v) => v,
    Err(e) => {
        send_reply(false, e, None, true);
        return;
    }
};
```

```rust
if !members.contains(&verified.issuer.as_ref().to_vec()) {
    send_reply(
        false,
        "the inviting member is no longer part of this network — this \
                 invite is permanently dead; ask a current member for a fresh one"
            .into(),
        None,
        true,
    );
    return;
}
```

- [ ] **Step 2: Build + clippy the touched crate**

Run: `ops/build-with.sh cargo clippy -p ducktape-node --tests --no-deps`
Expected: clean (warnings only from pre-existing debt, none from these lines).

- [ ] **Step 3: Commit**

```bash
git add bin/node/src/validator/run/ingress.rs
git commit -m "fix(node): permanent lobby rejects (bad crypto, dead issuer) reply fatal"
```

---

### Task 4: Joiner lobby-phase deadline — the universal backstop

First contact has a 90s window (`wiring.rs:298`); the lobby announce loop has NO ceiling. Any stall the member never answers fatally (mute member, dropped replies, undecodable replies after Task 2) spins forever. Add a hard deadline: a FRESH joiner (no standing) that announces for `JOIN_LOBBY_DEADLINE` without standing landing exits FATAL. A restart-with-standing keeps its retry-forever behavior (it is a restore, not a join).

**Files:**
- Modify: `bin/node/src/replica/wiring.rs` — the lobby announce loop (the task that periodically sends `lobby::join_request(..)` on `lobby_tx`; find it with `grep -n "join_request(" bin/node/src/replica/wiring.rs`, below the `lobby_replies` drain task at :367). The `restart_with_standing: bool` flag already exists in this scope (used at `wiring.rs:317`).

**Interfaces:**
- Consumes: the existing announce loop + `restart_with_standing`.
- Produces: `JOIN_LOBBY_DEADLINE` const; a `FATAL:` log line (the app's phase classifier keys on `FATAL`, `app/src-tauri/src/workspaces/phase.rs:39-42`).

- [ ] **Step 1: Add the deadline to the announce loop**

At the top of `wiring.rs` near the other consts, and in the announce loop body:

```rust
/// a FRESH joiner that has announced this long without standing landing is
/// stuck for a reason no member will ever cure (mute lobby, undecodable
/// replies, redemption permanently refused upstream) — stop loudly instead
/// of spinning forever. restart-with-standing is exempt: that path is a
/// restore and legitimately retries indefinitely.
const JOIN_LOBBY_DEADLINE: std::time::Duration = std::time::Duration::from_secs(300);
```

```rust
let announce_started = std::time::Instant::now();
loop {
    // ... existing announce send + sleep ...
    if !restart_with_standing && announce_started.elapsed() > JOIN_LOBBY_DEADLINE {
        eprintln!(
            "[node {label}] FATAL: no standing after {}s of announcing — a \
             member reply above names the reason if one arrived; otherwise \
             the lobby is unreachable or the members run an incompatible \
             binary. ask the inviter for a fresh invite and re-join.",
            JOIN_LOBBY_DEADLINE.as_secs()
        );
        std::process::exit(1);
    }
}
```

Anchor exactly as the loop is written (it may be a `select!` or an interval tick — put the check once per announce iteration). If `restart_with_standing` is not in scope in that task, thread the existing boolean in via the closure like the first-contact task does (`wiring.rs:313-335`).

- [ ] **Step 2: Verify the success path is untouched**

Run: `ops/build-with.sh cargo test -p ducktape-node --test join_request_e2e --test live_admission_e2e -- --nocapture`
Expected: PASS — a normal auto-redeemed join lands standing in well under 300s, and the reuse test's fatal reply beats the deadline. (The deadline itself is deliberately not e2e-tested: a 300s stall test is CI poison; the reply-driven fatal path is covered, and the deadline is a two-line guard.)

- [ ] **Step 3: Commit**

```bash
git add bin/node/src/replica/wiring.rs
git commit -m "fix(node): hard 5-minute deadline on the fresh-join lobby announce phase"
```

---

### Task 5: Gates, PR

- [ ] **Step 1: Full gate run on touched crates**

```bash
ops/build-with.sh cargo clippy -p ducktape-node --tests --no-deps
ops/build-with.sh cargo test -p ducktape-node lobby
ops/build-with.sh cargo test -p ducktape-node --test join_request_e2e --test invite_e2e --test live_admission_e2e --test join_request_e2e
```
Expected: all green. (Touch a `.rs` file first if cargo would serve a cached, warning-free build — cached runs pass vacuously.)

- [ ] **Step 2: PR against dev**

PR title: `fix(node): fail-loud invite redemption — strict replies, fatal audit, join deadline`. Body: root-cause finding from Task 1 Step 2 (or "behavior already correct with current binaries; hardening closes the old-binary and mute-member stalls"), the flag-day note (pre-cutover member replies no longer decode), and the reject→fatal table from Task 3.

```bash
git push -u origin fail-loud-redemption
gh pr create --base dev --title "fix(node): fail-loud invite redemption" --body "..."
```
