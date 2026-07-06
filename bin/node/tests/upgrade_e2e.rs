//! real-socket end-to-end proof of the height-gated, no-downtime node upgrade —
//! the CULMINATION of the upgrade module + forge v2 dual-path work (plan Phase 9).
//!
//! `cluster_upgrade` (the headline) drives REAL `ducktape-node` OS processes
//! through the WHOLE mechanism on one cluster, over the same json-lines rpc + the
//! greppable transition markers the runtime emits. The sequence:
//! seed committed FORGE state so the v2 root flip is OBSERVABLE; drive governance
//! `ScheduleUpgrade { to_version: 2, activation_height: H }` to passing; watch every
//! validator's `ReadinessSignaller` auto-emit `SignalReady`; watch readiness reach
//! `R == n` (the pre-boundary `upgrade armed …` marker); cross `H` (the `upgrade
//! activated … version=2 …` marker on EVERY node). Then it asserts the properties:
//!
//! - (a) every honest node AGREES on the app-hash at/after `H` (no fork).
//! - (b) the FORGE module root CHANGED at `H` (the v2 layout actually took effect).
//! - (c) no honest node halted (statuses keep answering; height advances past `H`).
//! - (d) the pending slot CLEARED (Advance reconciliation) so a SECOND upgrade is
//!   schedulable.
//! - (e) a validator restarted across `H` recovers the IDENTICAL app-hash
//!   (version-aware recovery replay).
//! - (f) a fresh state-sync joiner across `H` rebuilds the IDENTICAL app-hash
//!   (version-aware install).
//!
//! plus a live `ducktape-node upgrade-status` CLI read against a scheduled net.
//!
//! NOTE on the mixed-old/new-binary leg: a single `cargo test` links ONE node
//! binary (`CARGO_BIN_EXE_ducktape-node`, `MAX_PROTOCOL_VERSION = 3`), so a true
//! mixed v1/v2 handshake cannot be spawned here. The structurally-load-bearing
//! property it would assert — version gating rides the app/consensus payload, not
//! the p2p handshake namespace `sha256(scheme ‖ validators)` — is covered by the
//! design doc (`docs/superpowers/plans/2026-07-04-no-downtime-node-upgrade-plan.md`,
//! Phase 9 `upgrade_mixed_binary_no_partition`) and by the below-`H` byte-identical
//! inertness the forge unit tests prove.

mod common;

use std::time::{Duration, Instant};

use common::{Cluster, poll_until, serial};
use directory::{DirMsg, DirQuery, DirReply};
use forge::{ForgeMsg, ForgeQuery, ForgeReply};
use governance::{GovAction, GovMsg, GovQuery, GovReply, ProposalStatus};
use upgrade::{UpgradeQuery, UpgradeReply, UpgradeStatus};

/// convergence / boundary-crossing budget: mesh formation, readiness rounds, and
/// filler-driven view advancement are real-time on a loaded CI core; polls exit
/// early, so generosity is free.
const CONVERGE: Duration = Duration::from_secs(180);
/// budget for one submitted op to finalize and become readable elsewhere.
const FINALIZE: Duration = Duration::from_secs(60);
/// the first-attempt scheduled-activation lead (blocks). `schedule_upgrade` doubles
/// it on a min-lead abort until it sticks, so this only needs to cover a typical
/// ceremony's height growth to avoid retries; it is otherwise self-correcting.
/// calibrated to the GATED heartbeat regime (~1-3 blocks/s while a ceremony
/// runs): a ceremony grows height by tens of blocks, and the boundary is then
/// crossed by the filler pump at a few hundred views per CONVERGE window — 800
/// (the old value, tuned when unconditional nops kept blocks fast) no longer
/// fits the budget.
const UPGRADE_LEAD: u64 = 200;

/// this node's finalized height via the status rpc (`None` before the first block).
fn height(cluster: &Cluster, idx: usize) -> Option<u64> {
    cluster.status(idx)["height"].as_u64()
}

/// the forge module's committed root hex from the status projection — the clean
/// per-module witness that the v2 layout recomputed the root at `H`.
fn forge_root(cluster: &Cluster, idx: usize) -> Option<String> {
    cluster.status(idx)["modules"]["forge"]
        .as_str()
        .map(str::to_string)
}

/// this repo's committed HEAD hex on `idx`, `None` until the seed commit finalizes.
fn forge_head(cluster: &Cluster, idx: usize, repo: &str) -> Option<String> {
    let reply = cluster.query(
        idx,
        "forge",
        &forge::encode_query(&ForgeQuery::HeadOf { repo: repo.into() }),
    )?;
    match forge::decode_reply(&reply).ok()? {
        ForgeReply::Head(head) => head,
        _ => None,
    }
}

/// the upgrade module's committed status projection on `idx`.
fn upgrade_status(cluster: &Cluster, idx: usize) -> Option<UpgradeStatus> {
    let reply = cluster.query(
        idx,
        "upgrade",
        &upgrade::encode_query(&UpgradeQuery::Status),
    )?;
    let UpgradeReply::Status(st) = upgrade::decode_reply(&reply).ok()?;
    Some(st)
}

/// a directory key's value on `idx`, `None` until the op finalizes there.
fn dir_value(cluster: &Cluster, idx: usize, key: &str) -> Option<String> {
    let reply = cluster.query(
        idx,
        "directory",
        &directory::encode_query(&DirQuery::Get { key: key.into() }),
    )?;
    match directory::decode_reply(&reply) {
        Ok(DirReply::Value(v)) => v,
        _ => None,
    }
}

/// the node's live app-hash hex via the status rpc.
fn app_hash(cluster: &Cluster, idx: usize) -> String {
    cluster.status(idx)["app_hash"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn proposal_status(cluster: &Cluster, idx: usize, id: &str) -> Option<(ProposalStatus, usize)> {
    let reply = cluster.query(
        idx,
        "governance",
        &governance::encode_query(&GovQuery::Proposal {
            proposal_id: id.into(),
        }),
    )?;
    match governance::decode_reply(&reply) {
        Ok(GovReply::Proposal(Some(view))) => Some((view.status, view.votes.len())),
        _ => None,
    }
}

/// poll `pred` until it is true or `timeout` elapses; returns whether it held. the
/// non-panicking counterpart of [`poll_until`] — for probing an outcome that may
/// legitimately not occur (a governance Execute that aborts on a module gate).
fn wait_pred(timeout: Duration, mut pred: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if pred() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// run ONE propose -> 2-of-3 vote -> execute ceremony for `action` (nodes 0+1 are a
/// strict majority of the 3-validator set). returns once the Execute op has been
/// submitted — the CALLER checks whether it took effect (a passing proposal whose
/// module follow-up aborts stays Open, so success is defined by the follow-up's
/// observable effect, not the proposal status).
fn run_ceremony(cluster: &Cluster, proposal_id: &str, action: GovAction) {
    cluster.submit(
        0,
        "governance",
        &governance::encode_msg(&GovMsg::Propose {
            proposal_id: proposal_id.into(),
            action,
            voting_period: 600_000, // consensus-time ms; far past test end
        }),
    );
    // wait_pred + assert (not poll_until) so a timeout SHOWS the node logs —
    // "proposal never opened" is otherwise blind to which lane wedged.
    let opened = wait_pred(FINALIZE, || {
        proposal_status(cluster, 1, proposal_id)
            .is_some_and(|(s, _)| s == ProposalStatus::Open)
    });
    assert!(
        opened,
        "proposal {proposal_id} never opened;\n{}",
        cluster.all_log_tails(60)
    );
    let vote = governance::encode_msg(&GovMsg::Vote {
        proposal_id: proposal_id.into(),
        approve: true,
    });
    cluster.submit(0, "governance", &vote);
    cluster.submit(1, "governance", &vote);
    poll_until("both ballots to land", FINALIZE, || {
        proposal_status(cluster, 1, proposal_id).filter(|(_, v)| *v == 2)
    });
    cluster.submit(
        1,
        "governance",
        &governance::encode_msg(&GovMsg::Execute {
            proposal_id: proposal_id.into(),
        }),
    );
}

/// schedule a node upgrade ROBUSTLY against the fast, load-variable block rate.
///
/// the upgrade module's `MIN_UPGRADE_LEAD` gate REJECTS a `Schedule` whose
/// `activation_height` is not strictly beyond the EXECUTE block's height — and a
/// rejected governance follow-up ABORTS the whole Execute block, so the proposal
/// silently stays Open and no pending upgrade appears. finalized height self-advances
/// briskly at a load-dependent rate, so the height at execute time cannot be predicted
/// at propose time. this self-corrects: propose with a lead; if the Execute aborts
/// (pending never appears), DOUBLE the lead and retry with a fresh proposal from the
/// current (higher) height — the only abort cause here is min-lead (monotonicity and
/// at-most-one hold by construction), so doubling always converges.
///
/// returns the `activation_height` that stuck (its margin over the execute height is
/// bounded by one ceremony's growth, so the caller can cross it briskly).
fn schedule_upgrade(cluster: &Cluster, name: &str, to_version: u32, start_lead: u64) -> u64 {
    let mut lead = start_lead;
    for attempt in 0..6u32 {
        let base = height(cluster, 0).expect("finalized height before scheduling");
        let activation_height = base + lead;
        let pid = format!("sched-{name}-a{attempt}");
        run_ceremony(
            cluster,
            &pid,
            GovAction::ScheduleUpgrade {
                name: name.into(),
                activation_height,
                to_version,
            },
        );
        // did the schedule take? the module's pending slot now names this upgrade.
        let took = wait_pred(Duration::from_secs(30), || {
            upgrade_status(cluster, 0)
                .and_then(|s| s.pending)
                .is_some_and(|p| p.name == name && p.to_version == to_version)
        });
        if took {
            // return the COMMITTED activation_height — if an earlier attempt's
            // schedule is the one that actually stuck (a slow-to-appear pending), its
            // height is authoritative over this attempt's locally-computed one.
            return upgrade_status(cluster, 0)
                .and_then(|s| s.pending)
                .map(|p| p.activation_height)
                .expect("pending present after taking");
        }
        // Execute aborted on the min-lead gate (height raced past activation): the
        // proposal is abandoned Open; bump the lead and retry from the new height.
        lead *= 2;
    }
    panic!(
        "could not schedule upgrade {name} after retries (final lead {lead});\n{}",
        cluster.all_log_tails(40)
    );
}

/// push deterministically-applied idempotent directory fillers until `done` —
/// finalized views only advance with ops, so an idle net would park at the
/// armed boundary. ONE FILLER LANE PER VALIDATOR (three concurrent rpc
/// submitters, 25ms cadence each): concurrent submit pressure is exactly the
/// load that used to freeze the pump — the drain arm starved behind the rpc
/// arm's per-iteration timer reset, so heights froze and the cutover (drain-
/// driven) never fired. with the pump's absolute drain deadline that wedge is
/// fixed, and running the crossing under this load keeps it fixed: a
/// regression starves the drain again and this pump times out loudly.
fn push_until(cluster: &Cluster, what: &str, mut done: impl FnMut() -> bool) {
    use std::sync::atomic::{AtomicBool, Ordering};
    let deadline = Instant::now() + CONVERGE;
    let stop = AtomicBool::new(false);
    let timed_out = std::thread::scope(|s| {
        for lane in 0..3usize {
            let stop = &stop;
            s.spawn(move || {
                let mut filler = 0u32;
                while !stop.load(Ordering::Relaxed) {
                    filler += 1;
                    let payload =
                        directory::encode_msg(&directory::DirMsg::Set {
                            key: format!("upgrade-filler-l{lane}-{filler}"),
                            value: "x".into(),
                        });
                    let _ = cluster.rpc(
                        lane,
                        serde_json::json!({
                            "cmd": "submit",
                            "target": "directory",
                            "payload_hex": common::hex(&payload),
                        }),
                    );
                    std::thread::sleep(Duration::from_millis(25));
                }
            });
        }
        loop {
            if done() {
                stop.store(true, Ordering::Relaxed);
                return false;
            }
            if Instant::now() >= deadline {
                stop.store(true, Ordering::Relaxed);
                return true;
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    });
    assert!(
        !timed_out,
        "timed out waiting for {what};\n{}",
        cluster.all_log_tails(60)
    );
}

/// [`push_until`], but every lane RECORDS the filler keys the rpc ACKED
/// (`ok: true`) — the accept-contract witness for a boundary crossing: an
/// acked op may finalize LATE (the cutover carries accepted-but-unresolved
/// frames into the new epoch), but it may never vanish. returns the acked
/// keys for the caller's post-crossing presence assert.
fn push_tracked_until(
    cluster: &Cluster,
    what: &str,
    mut done: impl FnMut() -> bool,
) -> Vec<String> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    let deadline = Instant::now() + CONVERGE;
    let stop = AtomicBool::new(false);
    let acked = Mutex::new(Vec::new());
    let timed_out = std::thread::scope(|s| {
        for lane in 0..3usize {
            let stop = &stop;
            let acked = &acked;
            s.spawn(move || {
                let mut filler = 0u32;
                while !stop.load(Ordering::Relaxed) {
                    filler += 1;
                    let key = format!("crossing-l{lane}-{filler}");
                    let payload =
                        directory::encode_msg(&directory::DirMsg::Set {
                            key: key.clone(),
                            value: "x".into(),
                        });
                    let reply = cluster.rpc(
                        lane,
                        serde_json::json!({
                            "cmd": "submit",
                            "target": "directory",
                            "payload_hex": common::hex(&payload),
                        }),
                    );
                    if reply["ok"] == true {
                        acked.lock().expect("acked lock").push(key);
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
            });
        }
        loop {
            if done() {
                stop.store(true, Ordering::Relaxed);
                return false;
            }
            if Instant::now() >= deadline {
                stop.store(true, Ordering::Relaxed);
                return true;
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    });
    assert!(
        !timed_out,
        "timed out waiting for {what};\n{}",
        cluster.all_log_tails(60)
    );
    acked.into_inner().expect("acked lock")
}

/// the cross-node app-hash agreement witness, robust to the concurrent push
/// lanes: for a few seconds after [`push_until`] returns, each node is still
/// draining its own lane's accepted-but-unfinalized backlog, so instantaneous
/// app-hash reads legitimately skew. poll until every node reports the SAME
/// hash — equality at a shared boundary IS the no-fork property — and return
/// the agreed value.
fn settled_app_hash(cluster: &Cluster, nodes: usize) -> String {
    poll_until("cross-node app hashes to settle equal", FINALIZE, || {
        let h0 = app_hash(cluster, 0);
        ((!h0.is_empty()) && (1..nodes).all(|i| app_hash(cluster, i) == h0)).then_some(h0)
    })
}

#[test]
fn cluster_upgrade() {
    let _serial = serial();
    // mesh of 4 (node 3 is the future state-sync joiner), consensus subset of 3.
    let mut cluster = Cluster::new(&[0, 1, 2, 3], &[0, 1, 2]);

    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));
    cluster.spawn(1);
    cluster.spawn(2);

    // liveness: every validator applied the startup ops. the marker samples
    // its hash at a node-local drain boundary (announces on a provider host
    // skew the sample point), so the no-fork witness is settled_app_hash.
    for i in 0..3 {
        cluster.wait_marker(i, "converged app_hash=", CONVERGE);
    }
    settled_app_hash(&cluster, 3);

    // 1. seed committed FORGE state so the module root is NON-ZERO — only then is
    //    the v2 recomposition at H observable (an empty forge namespace roots to
    //    ZERO under both layouts).
    cluster.submit(
        0,
        "forge",
        &forge::encode_msg(&ForgeMsg::Commit {
            repo: "demo".into(),
            path: "README.md".into(),
            content: "forge v1 committed state".into(),
            message: "seed".into(),
        }),
    );
    poll_until("forge seed commit to finalize on node 1", FINALIZE, || {
        forge_head(&cluster, 1, "demo")
    });
    // quiesce so nothing is mid-drain, then capture the BASELINE forge root (v1).
    std::thread::sleep(Duration::from_secs(2));
    let forge_root_pre = forge_root(&cluster, 0).expect("forge root pre-H");
    assert_eq!(
        forge_root(&cluster, 1),
        Some(forge_root_pre.clone()),
        "forge root must agree cross-node before H"
    );

    // 2. schedule the upgrade to protocol v2 at a future height H (self-correcting
    //    against the fast, load-variable block rate — see `schedule_upgrade`).
    let activation_height = schedule_upgrade(&cluster, "forge-v2", 2, UPGRADE_LEAD);

    // 3. every validator's ReadinessSignaller auto-emits SignalReady (this binary
    //    is MAX_PROTOCOL_VERSION=3, so the signal is truthful).
    for i in 0..3 {
        cluster.wait_marker(i, "signaled ready name=forge-v2", CONVERGE);
    }

    // 4. readiness reaches R == n -> the pre-boundary ARM marker on every node.
    for i in 0..3 {
        cluster.wait_marker(i, "upgrade armed name=forge-v2 to_version=2", CONVERGE);
    }

    // upgrade-status CLI leg: a live read against the scheduled net reports the
    // pending upgrade, readiness count, and armed verdict.
    let cfg0 = cluster.config_file(0);
    let cfg0s = cfg0.to_str().expect("utf-8 config path");
    let (ok, out) = cluster.run_verb(&["upgrade-status", "--config", cfg0s]);
    assert!(ok, "upgrade-status CLI failed:\n{out}");
    assert!(
        out.contains("pending: name=forge-v2") && out.contains("to_version=2"),
        "upgrade-status must report the pending upgrade:\n{out}"
    );
    assert!(
        out.contains("armed (R == n): true"),
        "upgrade-status must report the armed verdict once R==n:\n{out}"
    );

    // 5. cross H (fillers advance finalized views) -> the ACTIVATION marker on
    //    every validator: the version cutover fired and forge flipped to v2.
    //    the lanes TRACK every acked filler — the boundary-crossing accept
    //    contract is asserted at (a') below.
    let acked = push_tracked_until(&cluster, "the upgrade to activate on every validator", || {
        (0..3).all(|i| {
            cluster
                .marker(i, "upgrade activated name=forge-v2 version=2")
                .is_some()
        })
    });
    let activated: Vec<String> = (0..3)
        .map(|i| cluster.wait_marker(i, "upgrade activated name=forge-v2 version=2 at height", CONVERGE))
        .collect();
    // the activation height agrees across nodes (one deterministic boundary).
    assert_eq!(activated[0], activated[1], "activation height fork 0 vs 1");
    assert_eq!(activated[0], activated[2], "activation height fork 0 vs 2");

    // (c) NO HALT: the new epoch must keep finalizing. a version-only cutover
    //     DISCARDS the frames at the boundary view (then re-proposes the
    //     locally-accepted ones into the new epoch — the boundary carry), so
    //     finalized height sits below H until the respawned epoch produces
    //     fresh blocks — push a directory op and require it to apply past the
    //     boundary on ANOTHER node (proves the post-H engine finalizes, and
    //     drives height past H).
    //     submit ONCE, then poll reads only: an acked op SURVIVES the boundary
    //     now (the cutover carries it), so the old defensive resubmit-per-poll
    //     loop is just spam that deepens the post-cutover queue the carried
    //     backlog is already draining through (one frame per block).
    cluster.submit(
        0,
        "directory",
        &directory::encode_msg(&DirMsg::Set {
            key: "post-h-liveness".into(),
            value: "alive".into(),
        }),
    );
    poll_until("the new epoch to finalize a post-H op", CONVERGE, || {
        dir_value(&cluster, 2, "post-h-liveness")
    });
    let post_h = height(&cluster, 0).expect("finalized height after H");
    assert!(
        post_h >= activation_height,
        "height {post_h} never reached activation height {activation_height} after the new epoch resumed"
    );

    // (a) NO FORK: every honest node agrees on the app-hash at/after H (the
    // settle poll rides out the push lanes' still-draining backlog).
    settled_app_hash(&cluster, 3);

    // (a') NO ACKED OP LOST: every filler the rpc acked during the crossing
    //      is readable after the boundary. accepted frames the old epoch
    //      never resolved — finalized past the discard ceiling, or still
    //      queued in the torn-down engine — are CARRIED into the new epoch
    //      by the cutover, so an ack may finalize late but may never vanish.
    //      polls generously: carried frames re-finalize behind the new
    //      epoch's queue.
    let mut remaining = acked;
    let total = remaining.len();
    let all_present = wait_pred(CONVERGE, || {
        remaining.retain(|k| dir_value(&cluster, 0, k).is_none());
        remaining.is_empty()
    });
    assert!(
        all_present,
        "accept contract BROKEN: {} of {total} acked crossing fillers never appeared post-H \
         (sample: {:?})",
        remaining.len(),
        &remaining[..remaining.len().min(5)]
    );
    println!("accept contract held: all {total} acked crossing fillers present post-H");

    // (b) THE UPGRADE DID SOMETHING: the forge module root recomputed under v2, so
    //     it differs from the byte-identical baseline captured before H (forge state
    //     is otherwise unchanged since the seed commit — the ONLY mover is the flip).
    let forge_root_post = forge_root(&cluster, 0).expect("forge root post-H");
    assert_ne!(
        forge_root_post, forge_root_pre,
        "forge module root did NOT change at H — the v2 layout never took effect"
    );
    assert_eq!(
        forge_root(&cluster, 2),
        Some(forge_root_post.clone()),
        "forge v2 root must agree cross-node"
    );
    // forge content survived the layout flip (v2 is a root/wire change, not a
    // data migration): the seed commit is still readable.
    assert!(
        forge_head(&cluster, 1, "demo").is_some(),
        "forge repo head must survive the v2 activation"
    );

    // (d.i) the pending slot CLEARED via the boundary Advance reconciliation, and
    //       current_version advanced to 2.
    let st = upgrade_status(&cluster, 0).expect("upgrade status post-H");
    assert!(
        st.pending.is_none(),
        "pending upgrade must clear after activation, got {:?}",
        st.pending
    );
    assert_eq!(st.current_version, 2, "current_version must be 2 after arming");
    // every node observed the clear (the greppable one-shot).
    for i in 0..3 {
        cluster.wait_marker(i, "upgrade cleared name=forge-v2", FINALIZE);
    }

    // (e) RESTART ACROSS H: kill a validator whose last committed height >= H, then
    //     respawn it. version-aware recovery replay must reconstruct app state and
    //     the node must re-converge to the SAME app-hash the live peers hold (no
    //     fork across the restart-over-a-boundary).
    cluster.kill(1);
    cluster.spawn(1);
    // the version-aware recovery replay ran (the manifest carried current_version=2,
    // so the forge branch is v2 on replay).
    cluster.wait_marker(1, "recovered app_hash=", CONVERGE);
    poll_until("restarted node 1 to re-converge with the live peers", CONVERGE, || {
        let a = app_hash(&cluster, 1);
        (!a.is_empty() && a == app_hash(&cluster, 0)).then_some(())
    });

    // (f) STATE-SYNC ACROSS H: a fresh joiner rebuilds the served boundary (past H)
    //     over the statesync channel and must compose the IDENTICAL app-hash —
    //     proving the served v2 snapshot installs and roots under the v2 layout.
    std::thread::sleep(Duration::from_secs(2));
    let served = app_hash(&cluster, 0);
    assert_eq!(app_hash(&cluster, 2), served, "server nodes disagree before sync");
    let (ok, log) = cluster.run_sync_only(3, Duration::from_secs(120));
    assert!(ok, "sync-only joiner across H failed:\n{log}");
    let synced = log
        .lines()
        .find_map(|l| l.split("synced app_hash=").nth(1))
        .expect("joiner printed a synced app-hash")
        .trim();
    assert_eq!(
        synced, served,
        "state-sync joiner across H rebuilt a DIFFERENT app-hash"
    );

    // (d.ii) with the slot free, a SECOND upgrade is schedulable — the at-most-one
    //        pending rule no longer rejects it (proving the Advance freed the slot).
    //        `schedule_upgrade` only returns once the pending slot names forge-v3,
    //        so a successful return IS the acceptance proof.
    let _ = schedule_upgrade(&cluster, "over-max-4", 4, UPGRADE_LEAD);
    // (test ends here; the second upgrade never arms — MAX_PROTOCOL_VERSION=3 < 4,
    // so no node signals ready — proving a truthful binary refuses to lie.)
}

/// ADVERSARIAL cluster leg: an upgrade scheduled to a `to_version` STRICTLY ABOVE
/// every node's `MAX_PROTOCOL_VERSION` must ABORT cleanly at `H` with ZERO network
/// downtime, then a subsequent SUPPORTED upgrade must still arm and activate. one
/// leg, four properties the happy path never exercises:
///
///   1. TRUTHFUL READINESS — no honest node ever emits `SignalReady` for a version
///      it cannot execute (the `ReadinessSignaller.decide` `to_version > max` gate),
///      so readiness stays `< n` and the arm verdict is never reached.
///   2. CLEAN ABORT — at `H` the boundary `Advance` clears the pending slot
///      (`upgrade cleared`) and the orchestrator cutover reads the ABORT verdict
///      (`upgrade aborted … unmet readiness`); `current_version` is UNCHANGED.
///   3. NO DOWNTIME THROUGH AN ABORT — the network keeps finalizing across `H`
///      (a post-abort op applies on another node; height passes `H`) and every
///      honest node still agrees on the app-hash (no halt, no fork).
///   4. RESCHEDULE — with the slot freed and `current_version` still `0`, a
///      SUPPORTED `to_version = 2` schedule arms (`R == n`) and ACTIVATES.
#[test]
fn cluster_upgrade_aborts_on_unmet_quorum() {
    let _serial = serial();
    // a 3-validator mesh — the abort proof needs no state-sync joiner.
    let mut cluster = Cluster::new(&[0, 1, 2], &[0, 1, 2]);

    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));
    cluster.spawn(1);
    cluster.spawn(2);

    // liveness via the markers; the no-fork witness is settled_app_hash
    // (the marker samples at a node-local drain boundary — see cluster_upgrade).
    for i in 0..3 {
        cluster.wait_marker(i, "converged app_hash=", CONVERGE);
    }
    settled_app_hash(&cluster, 3);

    // 1. schedule an upgrade to a to_version STRICTLY ABOVE MAX_PROTOCOL_VERSION
    //    (=2 in bin/node), so NO truthful node can ever signal ready for it. the
    //    SCHEDULE itself is version-agnostic (only monotonicity/lead/at-most-one
    //    gate it), so it arms a pending slot exactly like a supported one.
    const OVER_MAX: u32 = 4; // > MAX_PROTOCOL_VERSION=3
    let abort_height = schedule_upgrade(&cluster, "over-max", OVER_MAX, UPGRADE_LEAD);

    // TRUTHFUL READINESS: the pending upgrade is live, but the module's armed
    // verdict must stay false — a supported binary refuses to signal a version it
    // cannot run. give the signaller several pump ticks to (not) fire, then assert
    // zero readiness and an un-armed verdict on every node.
    std::thread::sleep(Duration::from_secs(3));
    for i in 0..3 {
        let st = upgrade_status(&cluster, i).expect("upgrade status pre-abort");
        assert_eq!(
            st.ready_count, 0,
            "node {i} recorded readiness for an over-max upgrade it cannot run"
        );
        assert!(
            !st.armed,
            "node {i} armed an over-max upgrade — a node signaled a version it cannot execute"
        );
        assert_eq!(st.current_version, 0, "current_version must be 0 pre-abort");
    }

    // baseline app-hash agreement before the boundary (the no-fork witness the
    // abort must preserve).
    let pre = app_hash(&cluster, 0);
    assert_eq!(app_hash(&cluster, 1), pre, "app-hash fork 0 vs 1 pre-abort");
    assert_eq!(app_hash(&cluster, 2), pre, "app-hash fork 0 vs 2 pre-abort");

    // 2. CLEAN ABORT at H: the boundary Advance clears the pending slot on every
    //    node (fillers advance finalized views across H).
    push_until(&cluster, "the over-max upgrade to clear at H", || {
        (0..3).all(|i| cluster.marker(i, "upgrade cleared name=over-max").is_some())
    });
    // the orchestrator cutover read the ABORT verdict (unmet readiness), NOT an arm.
    for i in 0..3 {
        cluster.wait_marker(i, "upgrade aborted name=over-max (unmet readiness)", CONVERGE);
    }
    // and NO node ever armed OR signaled it — the truthful-readiness proof bites.
    for i in 0..3 {
        assert!(
            cluster.marker(i, "signaled ready name=over-max").is_none(),
            "node {i} signaled ready for an over-max upgrade (untruthful readiness)"
        );
        assert!(
            cluster.marker(i, "upgrade armed name=over-max").is_none(),
            "node {i} armed an over-max upgrade that no node could signal for"
        );
    }
    // current_version is UNCHANGED (abort never flips) and the slot is free.
    for i in 0..3 {
        let st = upgrade_status(&cluster, i).expect("upgrade status post-abort");
        assert_eq!(st.current_version, 0, "abort must NOT flip current_version (node {i})");
        assert!(st.pending.is_none(), "abort must clear the pending slot (node {i})");
    }

    // 3. NO DOWNTIME: the network keeps finalizing across the aborted boundary — a
    //    fresh op applies on ANOTHER node and height advances past H.
    //    submit ONCE, then poll reads only (see the post-h-liveness note in
    //    cluster_upgrade: acked ops survive the boundary now, and the carried
    //    backlog drains one frame per block — resubmit-per-poll just spams it).
    cluster.submit(
        0,
        "directory",
        &directory::encode_msg(&DirMsg::Set {
            key: "post-abort-liveness".into(),
            value: "alive".into(),
        }),
    );
    poll_until("a post-abort op to finalize across H", CONVERGE, || {
        dir_value(&cluster, 2, "post-abort-liveness")
    });
    let post = height(&cluster, 0).expect("finalized height after the aborted H");
    assert!(
        post >= abort_height,
        "height {post} never reached the aborted activation height {abort_height} — the net stalled at H"
    );
    // confirm no honest node forked across the abort (settle poll rides out
    // the push lanes' still-draining backlog).
    settled_app_hash(&cluster, 3);

    // 4. RESCHEDULE: the slot is free and current_version is still 0, so a
    //    SUPPORTED to_version=2 upgrade now arms (R == n) and ACTIVATES — proving
    //    the abort left the mechanism fully re-armable (no residual pending/readiness).
    let arm_height = schedule_upgrade(&cluster, "recover-v2", 2, UPGRADE_LEAD);
    for i in 0..3 {
        cluster.wait_marker(i, "signaled ready name=recover-v2 to_version=2", CONVERGE);
        cluster.wait_marker(i, "upgrade armed name=recover-v2 to_version=2", CONVERGE);
    }
    push_until(&cluster, "the supported reschedule to activate on every node", || {
        (0..3).all(|i| cluster.marker(i, "upgrade activated name=recover-v2 version=2").is_some())
    });
    // the version-only cutover DISCARDS the frames at the boundary view, so the
    // in-block Advance that flips current_version only fires once the respawned
    // epoch finalizes a fresh block at/after H2. drive ops until the flip reconciles
    // to v2 on every node (if it ABORTED instead, this would time out — the bite).
    push_until(&cluster, "the reschedule flip to reconcile current_version=2", || {
        (0..3).all(|i| {
            upgrade_status(&cluster, i)
                .map(|s| s.current_version == 2 && s.pending.is_none())
                .unwrap_or(false)
        })
    });
    for i in 0..3 {
        let st = upgrade_status(&cluster, i).expect("upgrade status post-activation");
        assert_eq!(st.current_version, 2, "reschedule must activate to v2 (node {i})");
        assert!(st.pending.is_none(), "reschedule pending must clear on activation (node {i})");
    }
    // no fork across the reschedule activation (settle poll as above).
    settled_app_hash(&cluster, 3);
    let post2 = height(&cluster, 0).expect("finalized height after the armed H");
    assert!(post2 >= arm_height, "height {post2} never reached the armed activation height {arm_height}");
}
