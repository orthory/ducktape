//! ADVERSARIAL host-level proof for the no-downtime upgrade mechanism, standing
//! the REAL production modules (`upgrade` + `valset`) on a real `Host` and
//! driving them across an activation boundary `H`. this is the fast,
//! deterministic counterpart to the multi-process `cluster_upgrade*` legs — it
//! proves a property that is inherently single-node (a boundary self-transition
//! against a mid-window-changed valset) without the cost of OS processes or a
//! live mesh.
//!
//! MID-WINDOW ADMISSION ABORTS (`mid_window_admission_*`): the `R = n`
//! readiness denominator is recomputed against the LIVE boundary valset, not
//! the schedule-time set. Admitting a fresh (unready) validator between
//! `ScheduleUpgrade` and `H` — the spec's headline edge case — moves an
//! otherwise-armed upgrade to the clean ABORT: `current_version` stays put and
//! the pending slot clears. The control leg (same script, no admission) ARMS,
//! so the assertion bites: the admission is the sole cause of the abort.
//!
//! (the former `divergent_participant_*` leg proved that forge's dual-path
//! layout flip at `H` was fork-detectable. the dual path was deleted — every
//! module is version-invariant now — so the leg had nothing left to diverge
//! and was removed; the host-level activation properties it also touched live
//! in `upgrade_e2e.rs` and the kernel recovery tests' synthetic dual modules.)

use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use futures::executor::block_on;

use host::{BASELINE_VERSION, BlockContext, Host};
use sdk::{Msg, Origin};
use lifecycle::Lifecycle;
use lifecycle::{LifecycleMsg, encode_msg as lifecycle_encode};
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
        target: "lifecycle".into(),
        payload: lifecycle_encode(&LifecycleMsg::ScheduleUpgrade {
            name: name.into(),
            activation_height,
            to_version,
        }),
    }
}

fn signal(name: &str, to_version: u32) -> Msg {
    Msg {
        target: "lifecycle".into(),
        payload: lifecycle_encode(&LifecycleMsg::UpgradeReady {
            name: name.into(),
            to_version,
            commitment: None,
        }),
    }
}

fn current_version(host: &Host) -> u32 {
    let reply = block_on(host.query(
        "lifecycle",
        &lifecycle::encode_query(&lifecycle::LifecycleQuery::UpgradeStatus),
    ))
    .expect("status query");
    let lifecycle::LifecycleReply::UpgradeStatus(s) =
        lifecycle::decode_reply(&reply).expect("decode status")
    else {
        panic!("expected UpgradeStatus");
    };
    s.current_version
}

fn pending_is_none(host: &Host) -> bool {
    let reply = block_on(host.query(
        "lifecycle",
        &lifecycle::encode_query(&lifecycle::LifecycleQuery::UpgradeStatus),
    ))
    .expect("status query");
    let lifecycle::LifecycleReply::UpgradeStatus(s) =
        lifecycle::decode_reply(&reply).expect("decode status")
    else {
        panic!("expected UpgradeStatus");
    };
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

// ---- mid-window admission of an unready member aborts the flip ---------------

const ADMIT_H: u64 = 10;
const TO_VERSION: u32 = 2;

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
    host.register(Box::new(Lifecycle::new("lifecycle", "valset")));

    apply(&mut host, 1, BASELINE_VERSION, Origin::System, schedule("proto-v2", ADMIT_H, TO_VERSION));
    apply(&mut host, 2, BASELINE_VERSION, Origin::External(m0.clone()), signal("proto-v2", TO_VERSION));
    apply(&mut host, 3, BASELINE_VERSION, Origin::External(m1.clone()), signal("proto-v2", TO_VERSION));

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
    apply(&mut host, ADMIT_H, boundary_pv, Origin::External(m0), signal("proto-v2", TO_VERSION));
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
