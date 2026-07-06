//! ADVERSARIAL host-level proofs for the no-downtime upgrade mechanism, standing
//! the REAL production modules (`forge` + `upgrade` + `valset`) on a real `Host`
//! and driving them across an activation boundary `H`. these are the fast,
//! deterministic counterpart to the multi-process `cluster_upgrade*` legs — they
//! prove properties that are inherently single-node (a divergent participant's
//! LOCAL app-hash; a boundary self-transition against a mid-window-changed valset)
//! without the cost of OS processes or a live mesh.
//!
//! they cover two guarantees the happy-path cluster e2e never exercises:
//!
//!   1. NO-FORK UNDER A DIVERGENT PARTICIPANT (`divergent_participant_*`): a node
//!      that computes the OTHER forge branch at `H` (a straggler stuck on the v1
//!      layout while the boundary version is v2) derives a DIFFERENT app-hash than
//!      a correctly-activated node. Divergence is therefore detectable — the wrong
//!      hash is simply different, so in a real cluster it can never gather `2f+1`
//!      against the honest supermajority; it is out-voted and halts on
//!      `AppHashMismatch` rather than corrupting the agreed hash. The
//!      correctly-activated hash is deterministic (a second correct node matches
//!      it byte-for-byte).
//!
//!   2. MID-WINDOW ADMISSION ABORTS (`mid_window_admission_*`): the `R = n`
//!      readiness denominator is recomputed against the LIVE boundary valset, not
//!      the schedule-time set. Admitting a fresh (unready) validator between
//!      `ScheduleUpgrade` and `H` — the spec's headline edge case — moves an
//!      otherwise-armed upgrade to the clean ABORT: `current_version` stays put and
//!      the pending slot clears. The control leg (same script, no admission) ARMS,
//!      so the assertion bites: the admission is the sole cause of the abort.

use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use futures::executor::block_on;

use forge::{ForgeMsg, encode_msg as forge_encode};
use host::{BASELINE_VERSION, BlockContext, Host};
use sdk::{Msg, Origin, StateRoot};
use upgrade::Upgrade;
use upgrade::{UpgradeMsg, encode_msg as upgrade_encode};
use valset::Valset;
use valset::{ValsetMsg, ValsetQuery, ValsetReply, encode_msg as valset_encode};

/// this seed's deterministic ed25519 pubkey bytes — the identity a node derives.
fn key(seed: u64) -> Vec<u8> {
    PrivateKey::from_seed(seed).public_key().as_ref().to_vec()
}

/// apply one block at `height` under `pv` (the stamped `protocol_version`), from
/// `origin`, carrying `msg`. the host's drain injects the boundary `Advance` on
/// its own when the committed upgrade module is pending-at/after-`H`.
fn apply(host: &mut Host, height: u64, pv: u32, origin: Origin, msg: Msg) {
    block_on(host.submit_at(
        BlockContext {
            height,
            consensus_time: height,
            origin,
            protocol_version: pv,
        },
        msg,
    ))
    .expect("block applies");
}

fn schedule(name: &str, activation_height: u64, to_version: u32) -> Msg {
    Msg {
        target: "upgrade".into(),
        payload: upgrade_encode(&UpgradeMsg::Schedule {
            name: name.into(),
            activation_height,
            to_version,
        }),
    }
}

fn signal(name: &str, to_version: u32) -> Msg {
    Msg {
        target: "upgrade".into(),
        payload: upgrade_encode(&UpgradeMsg::SignalReady {
            name: name.into(),
            to_version,
            commitment: None,
        }),
    }
}

fn current_version(host: &Host) -> u32 {
    let reply = block_on(host.query(
        "upgrade",
        &upgrade::encode_query(&upgrade::UpgradeQuery::Status),
    ))
    .expect("status query");
    let upgrade::UpgradeReply::Status(s) =
        upgrade::decode_reply(&reply).expect("decode status");
    s.current_version
}

fn pending_is_none(host: &Host) -> bool {
    let reply = block_on(host.query(
        "upgrade",
        &upgrade::encode_query(&upgrade::UpgradeQuery::Status),
    ))
    .expect("status query");
    let upgrade::UpgradeReply::Status(s) =
        upgrade::decode_reply(&reply).expect("decode status");
    s.pending.is_none()
}

fn validator_count(host: &Host) -> usize {
    let reply = block_on(host.query("valset", &valset::encode_query(&ValsetQuery::Validators)))
        .expect("valset query");
    match valset::decode_reply(&reply).expect("decode valset") {
        ValsetReply::Validators(v) => v.len(),
        other => panic!("expected Validators, got {other:?}"),
    }
}

// ---- test 1: no-fork under a divergent participant --------------------------

const H: u64 = 10;
const TO_VERSION: u32 = 2; // FORGE_MULTIREPO_V2 — flips forge to the domain-separated root.

/// build a host (valset {m0,m1} + upgrade + forge), seed a forge commit so the
/// forge root is NON-ZERO (an empty namespace roots to ZERO under BOTH layouts),
/// schedule an upgrade to v2 at `H`, have BOTH members signal (so `R == n` and the
/// upgrade is armed), then cross `H`. `activate` models the fork: a CORRECT node
/// flips its dual-path forge to the boundary version (`set_active_version`), a
/// DIVERGENT straggler does not — it reaches `H` still computing the v1 layout.
/// either way the upgrade module's OWN state reconciles identically (its `Advance`
/// logic is version-invariant), so the SOLE divergence is forge's root.
///
/// returns (host post-`H`, the forge root captured just below `H`).
fn armed_forge_host(base: &std::path::Path, activate: bool) -> (Host, StateRoot) {
    let m0 = key(1);
    let m1 = key(2);

    let mut host = Host::new();
    let mut valset = Valset::new("valset");
    valset.insert(m0.clone());
    valset.insert(m1.clone());
    host.register(Box::new(valset));
    host.register(Box::new(Upgrade::new("upgrade", "valset")));
    host.register(Box::new(forge::Forge::init("forge", base.to_path_buf()).expect("forge init")));

    // seed committed forge state so the v2 recomposition at H is OBSERVABLE.
    apply(
        &mut host,
        0,
        BASELINE_VERSION,
        Origin::System,
        Msg {
            target: "forge".into(),
            payload: forge_encode(&ForgeMsg::Commit {
                repo: "demo".into(),
                path: "README.md".into(),
                content: "forge v1 committed state".into(),
                message: "seed".into(),
            }),
        },
    );
    // schedule (governance/system-authored) + both members signal -> armed.
    apply(&mut host, 1, BASELINE_VERSION, Origin::System, schedule("forge-v2", H, TO_VERSION));
    apply(&mut host, 2, BASELINE_VERSION, Origin::External(m0.clone()), signal("forge-v2", TO_VERSION));
    apply(&mut host, 3, BASELINE_VERSION, Origin::External(m1.clone()), signal("forge-v2", TO_VERSION));

    // the forge root just below H (baseline v1 layout, committed heads unchanged).
    let forge_pre = host.module_root("forge").expect("forge root pre-H");

    // ACTIVATION at H. a CORRECT node flips the dual-path branch selector to the
    // agreed boundary version; a straggler leaves it at baseline (v1). the stamped
    // protocol_version is the agreed derivation on BOTH (a straggler still dispatches
    // under the agreed version — its divergence is the un-flipped forge branch).
    let boundary_pv = block_on(host.effective_version(H));
    assert_eq!(boundary_pv, TO_VERSION, "the armed boundary must derive to_version");
    if activate {
        host.set_active_version(boundary_pv);
    }
    // a re-signal at H is idempotent; it merely carries the block whose drain
    // injects the boundary Advance (which reconciles the upgrade module's own state).
    apply(&mut host, H, boundary_pv, Origin::External(m0), signal("forge-v2", TO_VERSION));

    (host, forge_pre)
}

#[test]
fn divergent_participant_computes_a_different_app_hash() {
    let correct_dir = tempfile::TempDir::new().expect("correct tempdir");
    let straggler_dir = tempfile::TempDir::new().expect("straggler tempdir");
    let witness_dir = tempfile::TempDir::new().expect("witness tempdir");

    let (correct, correct_pre) = armed_forge_host(correct_dir.path(), true);
    let (straggler, straggler_pre) = armed_forge_host(straggler_dir.path(), false);

    // the seed produced an identical (deterministic, non-zero) forge root on both.
    assert_ne!(correct_pre, StateRoot::ZERO, "seed must give a non-zero forge root");
    assert_eq!(correct_pre, straggler_pre, "identical seed -> identical pre-H forge root");

    let correct_forge = correct.module_root("forge").expect("correct forge root");
    let straggler_forge = straggler.module_root("forge").expect("straggler forge root");

    // the correctly-activated node RECOMPUTED the forge root under the v2 layout...
    assert_ne!(
        correct_forge, correct_pre,
        "activation must move the forge root (the v2 layout took effect)"
    );
    // ...while the straggler, never having flipped, still holds the v1-layout root.
    assert_eq!(
        straggler_forge, straggler_pre,
        "the straggler never flipped — its forge root stays on the v1 layout"
    );
    // THE PROPERTY: the two forge roots differ, so the DIVERGENCE IS DETECTABLE.
    assert_ne!(
        correct_forge, straggler_forge,
        "a divergent participant must compute a DIFFERENT forge root at H"
    );

    // the divergence is LOCALIZED to forge: the upgrade module's own state (version
    // flipped to 2, pending cleared) and the valset reconcile IDENTICALLY on both —
    // the straggler is not "behind", it is running the WRONG forge branch.
    assert_eq!(current_version(&correct), TO_VERSION, "correct node flipped to v2");
    assert_eq!(current_version(&straggler), TO_VERSION, "straggler's upgrade module still flipped");
    assert_eq!(
        correct.module_root("upgrade"),
        straggler.module_root("upgrade"),
        "the version-invariant upgrade module must reconcile identically"
    );
    assert_eq!(
        correct.module_root("valset"),
        straggler.module_root("valset"),
        "valset is untouched by the flip"
    );

    // therefore the GLOBAL app-hashes differ — the straggler's block cannot match
    // the honest quorum's certificate; it is out-voted, never adopted.
    assert_ne!(
        correct.app_hash(),
        straggler.app_hash(),
        "the divergent app-hash must be distinguishable from the correct one"
    );

    // and the correctly-activated app-hash is DETERMINISTIC: an independent correct
    // node composes the byte-identical hash (so the honest side agrees on ONE hash).
    let (witness, _) = armed_forge_host(witness_dir.path(), true);
    assert_eq!(
        correct.app_hash(),
        witness.app_hash(),
        "two correctly-activated nodes must agree on the app-hash at H"
    );
}

// ---- test 2: mid-window admission of an unready member aborts the flip -------

const ADMIT_H: u64 = 10;

/// drive valset {m0,m1} + upgrade through an armed boundary at `ADMIT_H`, both
/// members signalling so the upgrade WOULD arm. when `admit_extra`, a THIRD
/// validator (`m2`) is admitted at height 4 (< H) and never signals — so at `H`
/// the boundary member set is {m0,m1,m2} with an unready member, which the shared
/// `effective_version` predicate the boundary `Advance` evaluates must read as
/// UNMET (`R < n`), driving the clean ABORT. returns the host just after `H`.
fn boundary_with_optional_admission(admit_extra: bool) -> Host {
    let m0 = key(1);
    let m1 = key(2);
    let m2 = key(3);

    let mut host = Host::new();
    let mut valset = Valset::new("valset");
    valset.insert(m0.clone());
    valset.insert(m1.clone());
    host.register(Box::new(valset));
    host.register(Box::new(Upgrade::new("upgrade", "valset")));

    apply(&mut host, 1, BASELINE_VERSION, Origin::System, schedule("forge-v2", ADMIT_H, TO_VERSION));
    apply(&mut host, 2, BASELINE_VERSION, Origin::External(m0.clone()), signal("forge-v2", TO_VERSION));
    apply(&mut host, 3, BASELINE_VERSION, Origin::External(m1.clone()), signal("forge-v2", TO_VERSION));

    if admit_extra {
        // a fresh validator admitted DURING the open upgrade window — governance
        // (module/system origin) drives valset membership. m2 runs an old binary
        // (or simply hasn't signaled), so it is dead weight against R = n.
        apply(
            &mut host,
            4,
            BASELINE_VERSION,
            Origin::System,
            Msg {
                target: "valset".into(),
                payload: valset_encode(&ValsetMsg::Join { key: m2.clone() }),
            },
        );
        assert_eq!(validator_count(&host), 3, "m2 must be admitted before H");
    } else {
        assert_eq!(validator_count(&host), 2, "no admission -> the schedule-time set");
    }

    // cross H: a re-signal carries the block whose drain injects the boundary
    // Advance, which arms-or-aborts against the LIVE boundary valset.
    let boundary_pv = block_on(host.effective_version(ADMIT_H));
    apply(&mut host, ADMIT_H, boundary_pv, Origin::External(m0), signal("forge-v2", TO_VERSION));
    host
}

#[test]
fn mid_window_admission_of_an_unready_member_aborts_the_flip() {
    // CONTROL: the exact same script with NO admission ARMS at H.
    let control = boundary_with_optional_admission(false);
    assert_eq!(
        current_version(&control),
        TO_VERSION,
        "control (no admission) must ARM: every boundary member signaled"
    );
    assert!(pending_is_none(&control), "control must clear the pending slot on arm");

    // ADVERSARIAL: admitting one unready validator mid-window is the SOLE change,
    // and it moves the identical boundary to the clean ABORT.
    let aborted = boundary_with_optional_admission(true);
    assert_eq!(
        current_version(&aborted),
        0,
        "a mid-window admission of an unready member must ABORT: current_version unchanged"
    );
    assert!(
        pending_is_none(&aborted),
        "abort must still clear the pending slot (the boundary Advance always reconciles)"
    );
}
