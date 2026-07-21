//! the merged lifecycle proof: BOTH the protocol-version path (was `upgrade`)
//! and the module-code path (was `modreg`) on one module, one root, one wire —
//! plus the combined-root and shared-`Advance` guarantees the merge introduces.

use super::*;
use sdk_testkit::TestCtx;

/// the sole host-routed read lifecycle makes: answers the valset `Validators`
/// query with `members`, every other query unsupported.
fn validators(members: Vec<Vec<u8>>) -> impl FnMut(&[u8]) -> Result<Vec<u8>, Error> {
    move |req| {
        let is_validators = matches!(
            valset::decode_query(req),
            Ok(valset::ValsetQuery::Validators)
        );
        if is_validators {
            return Ok(valset::encode_reply(&valset::ValsetReply::Validators(
                members.clone(),
            )));
        }
        Err(Error::QueryUnsupported)
    }
}

fn env_at(origin: Origin, height: u64, protocol_version: u32) -> sdk::Env {
    sdk::Env {
        protocol_version,
        height,
        consensus_time: 0,
        origin,
        me: "lifecycle".into(),
    }
}

/// a ctx over `origin`/`height` at protocol v0, valset defaulting to a single
/// member — the shape the old `TestCtx::new` produced.
fn ctx(origin: Origin, height: u64) -> TestCtx {
    TestCtx::with_env(env_at(origin, height, 0)).on_query("valset", validators(vec![member(1)]))
}

/// `ctx` at the protocol version where module admission is active.
fn ctx_at_admission(origin: Origin, height: u64) -> TestCtx {
    TestCtx::with_env(env_at(origin, height, ADMISSION_ACTIVATION_VERSION))
        .on_query("valset", validators(vec![member(1)]))
}

/// lifecycle-domain verb kept as a call-site method so the `.with_members`
/// sites read unchanged after the swap: it re-keys the valset reply.
trait WithMembers {
    fn with_members(self, members: Vec<Vec<u8>>) -> Self;
}
impl WithMembers for TestCtx {
    fn with_members(self, members: Vec<Vec<u8>>) -> Self {
        self.on_query("valset", validators(members))
    }
}

fn member(seed: u8) -> Vec<u8> {
    vec![seed; 32]
}
fn hash(seed: u8) -> Vec<u8> {
    vec![seed; CODE_HASH_LEN]
}
fn fresh() -> Lifecycle {
    Lifecycle::new("lifecycle", "valset")
}
fn msg(m: LifecycleMsg) -> Msg {
    Msg {
        target: "lifecycle".into(),
        payload: encode_msg(&m),
    }
}
fn run(lc: &mut Lifecycle, ctx: &mut TestCtx, m: &Msg) -> Result<(), Error> {
    futures::executor::block_on(lc.execute(ctx, m))
}
fn commit(lc: &mut Lifecycle) {
    futures::executor::block_on(lc.commit_block()).unwrap();
}

// ---- protocol-version path helpers ------------------------------------------

fn schedule_upgrade(name: &str, ah: u64, tv: u32) -> Msg {
    msg(LifecycleMsg::ScheduleUpgrade {
        name: name.into(),
        activation_height: ah,
        to_version: tv,
    })
}
fn cancel_upgrade(name: &str) -> Msg {
    msg(LifecycleMsg::CancelUpgrade { name: name.into() })
}
fn upgrade_ready(name: &str, tv: u32) -> Msg {
    upgrade_ready_with(name, tv, None)
}
fn upgrade_ready_with(name: &str, tv: u32, commitment: Option<Vec<u8>>) -> Msg {
    msg(LifecycleMsg::UpgradeReady {
        name: name.into(),
        to_version: tv,
        commitment,
    })
}
fn advance() -> Msg {
    msg(LifecycleMsg::Advance)
}
fn upgrade_status(lc: &Lifecycle, ctx: &TestCtx) -> UpgradeStatus {
    let reply = futures::executor::block_on(
        lc.query_with(ctx, &encode_query(&LifecycleQuery::UpgradeStatus)),
    )
    .unwrap();
    match decode_reply(&reply).unwrap() {
        LifecycleReply::UpgradeStatus(s) => s,
        other => panic!("expected UpgradeStatus, got {other:?}"),
    }
}

// ---- module-code path helpers -----------------------------------------------

fn register_module(lc: &mut Lifecycle, module_id: &str, code: u8) {
    let mut sys = ctx(Origin::System, 0);
    run(
        lc,
        &mut sys,
        &msg(LifecycleMsg::RegisterModule {
            module_id: module_id.into(),
            code_hash: hash(code),
        }),
    )
    .unwrap();
    commit(lc);
}
fn schedule_swap(module_id: &str, name: &str, ah: u64, code: u8) -> Msg {
    msg(LifecycleMsg::ScheduleSwap {
        name: name.into(),
        module_id: module_id.into(),
        activation_height: ah,
        code_hash: hash(code),
    })
}
fn schedule_register(module_id: &str, name: &str, ah: u64, code: u8) -> Msg {
    msg(LifecycleMsg::ScheduleRegister {
        name: name.into(),
        module_id: module_id.into(),
        activation_height: ah,
        code_hash: hash(code),
    })
}
fn cancel_swap(module_id: &str, name: &str) -> Msg {
    msg(LifecycleMsg::CancelSwap {
        name: name.into(),
        module_id: module_id.into(),
    })
}
fn swap_ready(module_id: &str, name: &str) -> Msg {
    msg(LifecycleMsg::SwapReady {
        name: name.into(),
        module_id: module_id.into(),
    })
}
/// drive the full single-member readiness latch for a pending swap.
fn make_swap_ready(lc: &mut Lifecycle, module_id: &str, name: &str) {
    let mut ext = ctx(Origin::External(member(1)), 0);
    run(lc, &mut ext, &swap_ready(module_id, name)).unwrap();
    commit(lc);
}
fn module_status(lc: &Lifecycle) -> Vec<ModuleCode> {
    let bytes = futures::executor::block_on(
        lc.query_with(&ctx(Origin::System, 0), &encode_query(&LifecycleQuery::ModuleStatus)),
    )
    .unwrap();
    match decode_reply(&bytes).unwrap() {
        LifecycleReply::ModuleStatus { modules } => modules,
        other => panic!("expected ModuleStatus, got {other:?}"),
    }
}
fn armed_at(lc: &Lifecycle, height: u64) -> Vec<ArmedSwap> {
    let bytes = futures::executor::block_on(lc.query_with(
        &ctx(Origin::System, 0),
        &encode_query(&LifecycleQuery::ArmedAt { height }),
    ))
    .unwrap();
    match decode_reply(&bytes).unwrap() {
        LifecycleReply::ArmedAt { swaps } => swaps,
        other => panic!("expected ArmedAt, got {other:?}"),
    }
}

// ============================================================================
// protocol-version path
// ============================================================================

#[test]
fn upgrade_schedule_and_cancel_origin_gate() {
    let mut lc = fresh();
    let mut ext = ctx(Origin::External(member(1)), 0);
    assert!(matches!(
        run(&mut lc, &mut ext, &schedule_upgrade("a", 10, 2)),
        Err(Error::Module(_))
    ));
    let mut gov = ctx(Origin::Module("governance".into()), 0);
    run(&mut lc, &mut gov, &schedule_upgrade("a", 10, 2)).unwrap();
    commit(&mut lc);
    assert!(lc.committed.pending_upgrade.is_some());
    let mut sys = ctx(Origin::System, 0);
    run(&mut lc, &mut sys, &cancel_upgrade("a")).unwrap();
    commit(&mut lc);
    assert!(lc.committed.pending_upgrade.is_none());
}

#[test]
fn upgrade_ready_origin_gate() {
    let mut lc = fresh();
    let mut sys = ctx(Origin::System, 0);
    run(&mut lc, &mut sys, &schedule_upgrade("a", 10, 2)).unwrap();
    commit(&mut lc);
    // module/system rejected.
    assert!(matches!(
        run(&mut lc, &mut sys, &upgrade_ready("a", 2)),
        Err(Error::Module(_))
    ));
    // non-member rejected.
    let mut bad = ctx(Origin::External(member(9)), 0);
    assert!(matches!(
        run(&mut lc, &mut bad, &upgrade_ready("a", 2)),
        Err(Error::Module(_))
    ));
    // member accepted.
    let mut good = ctx(Origin::External(member(1)), 0);
    run(&mut lc, &mut good, &upgrade_ready("a", 2)).unwrap();
    commit(&mut lc);
    assert!(lc.committed.upgrade_readiness.contains_key(&member(1)));
}

#[test]
fn upgrade_schedule_monotonic_min_lead_one_pending() {
    let mut lc = fresh();
    let mut sys = ctx(Origin::System, 0);
    // to_version 0 !> current 0.
    assert!(run(&mut lc, &mut sys, &schedule_upgrade("a", 10, 0)).is_err());
    // min lead: height 0 + 3 -> activation must be > 3.
    assert!(run(&mut lc, &mut sys, &schedule_upgrade("a", 3, 2)).is_err());
    run(&mut lc, &mut sys, &schedule_upgrade("a", 10, 2)).unwrap();
    commit(&mut lc);
    // a second pending rejected.
    assert!(run(&mut lc, &mut sys, &schedule_upgrade("b", 20, 3)).is_err());
}

#[test]
fn upgrade_advance_arm_and_abort() {
    let members = vec![member(1), member(2)];
    // ARM: both members signal.
    let mut lc = fresh();
    let mut sys = ctx(Origin::System, 0).with_members(members.clone());
    run(&mut lc, &mut sys, &schedule_upgrade("a", 10, 2)).unwrap();
    commit(&mut lc);
    for m in &members {
        let mut c = ctx(Origin::External(m.clone()), 0).with_members(members.clone());
        run(&mut lc, &mut c, &upgrade_ready("a", 2)).unwrap();
        commit(&mut lc);
    }
    let mut boundary = ctx(Origin::System, 10).with_members(members.clone());
    run(&mut lc, &mut boundary, &advance()).unwrap();
    commit(&mut lc);
    assert_eq!(lc.committed.current_version, 2, "armed flips version");
    assert!(lc.committed.pending_upgrade.is_none());
    assert!(lc.committed.upgrade_readiness.is_empty());

    // ABORT: only one of two signals.
    let mut lc = fresh();
    run(&mut lc, &mut sys, &schedule_upgrade("a", 10, 2)).unwrap();
    commit(&mut lc);
    let mut c1 = ctx(Origin::External(members[0].clone()), 0).with_members(members.clone());
    run(&mut lc, &mut c1, &upgrade_ready("a", 2)).unwrap();
    commit(&mut lc);
    run(&mut lc, &mut boundary, &advance()).unwrap();
    commit(&mut lc);
    assert_eq!(lc.committed.current_version, 0, "abort leaves version");
    assert!(lc.committed.pending_upgrade.is_none(), "abort clears pending");
}

// the boundary race: the nth SignalReady is finalized AS block H's own op, so it
// stages readiness=n in the SAME drain the injected Advance runs in. The Advance
// MUST decide over FROZEN committed (H-1) readiness = n-1 and ABORT.
#[test]
fn upgrade_same_block_late_signal_aborts_not_arms() {
    let members = vec![member(1), member(2)];
    let mut lc = fresh();
    let mut sys = ctx(Origin::System, 0).with_members(members.clone());
    run(&mut lc, &mut sys, &schedule_upgrade("a", 10, 2)).unwrap();
    commit(&mut lc);
    let mut c0 = ctx(Origin::External(members[0].clone()), 1).with_members(members.clone());
    run(&mut lc, &mut c0, &upgrade_ready("a", 2)).unwrap();
    commit(&mut lc);
    assert_eq!(lc.committed.upgrade_readiness.len(), 1);

    // block H: m1's signal stages readiness=n, Advance runs in the SAME drain.
    let mut c1 = ctx(Origin::External(members[1].clone()), 10).with_members(members.clone());
    run(&mut lc, &mut c1, &upgrade_ready("a", 2)).unwrap();
    let mut sysh = ctx(Origin::System, 10).with_members(members);
    run(&mut lc, &mut sysh, &advance()).unwrap();
    commit(&mut lc);
    assert_eq!(
        lc.committed.current_version, 0,
        "a same-block late nth signal must NOT flip: quorum was unmet at H-1"
    );
    assert!(lc.committed.pending_upgrade.is_none(), "aborts cleanly");
}

#[test]
fn upgrade_commitment_bound_ignores_wrong_artifacts() {
    let members = vec![member(1)];
    let name = "commit:clients-route-a";
    let mut lc = fresh();
    let mut sys = ctx(Origin::System, 0).with_members(members.clone());
    run(&mut lc, &mut sys, &schedule_upgrade(name, 10, 1)).unwrap();
    commit(&mut lc);
    let mut v = ctx(Origin::External(member(1)), 0).with_members(members.clone());
    // wrong-route commitment: stored but not counted / not armable.
    run(
        &mut lc,
        &mut v,
        &upgrade_ready_with(name, 1, Some(b"wrong".to_vec())),
    )
    .unwrap();
    commit(&mut lc);
    let ctx = ctx(Origin::System, 0).with_members(members.clone());
    assert_eq!(upgrade_status(&lc, &ctx).ready_count, 0);
    assert!(!upgrade_status(&lc, &ctx).armed);
    // exact route commitment arms.
    run(
        &mut lc,
        &mut v,
        &upgrade_ready_with(name, 1, Some(b"clients-route-a".to_vec())),
    )
    .unwrap();
    commit(&mut lc);
    assert_eq!(upgrade_status(&lc, &ctx).ready_count, 1);
    assert!(upgrade_status(&lc, &ctx).armed);
}

#[test]
fn effective_version_only_when_armed() {
    let members = vec![member(1), member(2)];
    let mut lc = fresh();
    let mut sys = ctx(Origin::System, 0).with_members(members.clone());
    run(&mut lc, &mut sys, &schedule_upgrade("a", 10, 2)).unwrap();
    commit(&mut lc);
    for m in &members {
        let mut c = ctx(Origin::External(m.clone()), 0).with_members(members.clone());
        run(&mut lc, &mut c, &upgrade_ready("a", 2)).unwrap();
        commit(&mut lc);
    }
    let ev = |height: u64, boundary: &[Vec<u8>]| {
        effective_version(
            height,
            lc.committed.current_version,
            lc.committed.pending_upgrade.as_ref(),
            boundary,
            |member| lc.committed.upgrade_readiness.contains_key(member),
        )
    };
    assert_eq!(ev(10, &members), 2);
    assert_eq!(ev(9, &members), 0);
    assert_eq!(ev(10, &[]), 0);
}

// ============================================================================
// module-code path
// ============================================================================

#[test]
fn register_and_schedule_origin_gate() {
    let mut lc = fresh();
    let mut ext = ctx(Origin::External(member(1)), 0);
    assert!(matches!(
        run(
            &mut lc,
            &mut ext,
            &msg(LifecycleMsg::RegisterModule {
                module_id: "hello".into(),
                code_hash: hash(1)
            })
        ),
        Err(Error::Module(_))
    ));
    let mut gov = ctx(Origin::Module("governance".into()), 0);
    run(
        &mut lc,
        &mut gov,
        &msg(LifecycleMsg::RegisterModule {
            module_id: "hello".into(),
            code_hash: hash(1),
        }),
    )
    .unwrap();
    commit(&mut lc);
    assert_eq!(module_status(&lc)[0].active_code_hash, hash(1));
    assert!(matches!(
        run(&mut lc, &mut ext, &schedule_swap("hello", "v2", 10, 2)),
        Err(Error::Module(_))
    ));
    run(&mut lc, &mut gov, &schedule_swap("hello", "v2", 10, 2)).unwrap();
    commit(&mut lc);
    assert!(module_status(&lc)[0].pending.is_some());
}

#[test]
fn advance_is_system_only() {
    let mut lc = fresh();
    register_module(&mut lc, "hello", 1);
    let mut gov = ctx(Origin::Module("governance".into()), 10);
    assert!(matches!(
        run(&mut lc, &mut gov, &advance()),
        Err(Error::Module(_))
    ));
}

#[test]
fn register_rejects_reregistration_and_bad_hash() {
    let mut lc = fresh();
    register_module(&mut lc, "hello", 1);
    let mut sys = ctx(Origin::System, 0);
    assert!(run(
        &mut lc,
        &mut sys,
        &msg(LifecycleMsg::RegisterModule {
            module_id: "hello".into(),
            code_hash: hash(9)
        })
    )
    .is_err());
    assert!(run(
        &mut lc,
        &mut sys,
        &msg(LifecycleMsg::RegisterModule {
            module_id: "other".into(),
            code_hash: vec![1, 2, 3]
        })
    )
    .is_err());
}

#[test]
fn schedule_swap_validation() {
    let mut lc = fresh();
    register_module(&mut lc, "hello", 1);
    let mut sys = ctx(Origin::System, 0);
    // unregistered module.
    assert!(run(&mut lc, &mut sys, &schedule_swap("ghost", "v2", 10, 2)).is_err());
    // min lead.
    assert!(run(&mut lc, &mut sys, &schedule_swap("hello", "v2", 3, 2)).is_err());
    // no-op swap.
    assert!(run(&mut lc, &mut sys, &schedule_swap("hello", "v2", 10, 1)).is_err());
    run(&mut lc, &mut sys, &schedule_swap("hello", "v2", 10, 2)).unwrap();
    commit(&mut lc);
    // second pending.
    assert!(run(&mut lc, &mut sys, &schedule_swap("hello", "v3", 20, 3)).is_err());
}

#[test]
fn swap_advance_activates_at_height_and_frees_slot() {
    let mut lc = fresh();
    register_module(&mut lc, "hello", 1);
    let mut sys = ctx(Origin::System, 0);
    run(&mut lc, &mut sys, &schedule_swap("hello", "v2", 10, 2)).unwrap();
    commit(&mut lc);
    make_swap_ready(&mut lc, "hello", "v2");

    // below activation: no-op.
    let root_before = lc.root();
    let mut below = ctx(Origin::System, 9);
    run(&mut lc, &mut below, &advance()).unwrap();
    commit(&mut lc);
    assert_eq!(lc.root(), root_before);
    assert_eq!(module_status(&lc)[0].active_code_hash, hash(1));

    // at activation.
    let mut at = ctx(Origin::System, 10);
    run(&mut lc, &mut at, &advance()).unwrap();
    commit(&mut lc);
    assert_eq!(module_status(&lc)[0].active_code_hash, hash(2));
    assert!(module_status(&lc)[0].pending.is_none());
}

#[test]
fn swap_readiness_gate() {
    let mut lc = fresh();
    register_module(&mut lc, "hello", 1);
    let mut sys = ctx(Origin::System, 0);
    run(&mut lc, &mut sys, &schedule_swap("hello", "v2", 10, 2)).unwrap();
    commit(&mut lc);
    // no readiness: never arms.
    assert!(armed_at(&lc, u64::MAX).is_empty());

    let two = vec![member(1), member(2)];
    let mut m1 = ctx(Origin::External(member(1)), 0).with_members(two.clone());
    run(&mut lc, &mut m1, &swap_ready("hello", "v2")).unwrap();
    commit(&mut lc);
    assert!(!module_status(&lc)[0].pending.clone().unwrap().ready);
    assert!(armed_at(&lc, 10).is_empty());

    let mut m2 = ctx(Origin::External(member(2)), 0).with_members(two);
    run(&mut lc, &mut m2, &swap_ready("hello", "v2")).unwrap();
    commit(&mut lc);
    assert!(module_status(&lc)[0].pending.clone().unwrap().ready);
    assert_eq!(armed_at(&lc, 10).len(), 1);
}

#[test]
fn swap_signal_gates_origin_and_identity() {
    let mut lc = fresh();
    register_module(&mut lc, "hello", 1);
    let mut sys = ctx(Origin::System, 0);
    run(&mut lc, &mut sys, &schedule_swap("hello", "v2", 10, 2)).unwrap();
    commit(&mut lc);
    assert!(run(&mut lc, &mut sys, &swap_ready("hello", "v2")).is_err());
    let mut stranger = ctx(Origin::External(member(9)), 0);
    assert!(run(&mut lc, &mut stranger, &swap_ready("hello", "v2")).is_err());
    let mut m1 = ctx(Origin::External(member(1)), 0);
    assert!(run(&mut lc, &mut m1, &swap_ready("hello", "vX")).is_err());
}

#[test]
fn swap_cancel_guards_and_clears() {
    let mut lc = fresh();
    register_module(&mut lc, "hello", 1);
    let mut sys = ctx(Origin::System, 0);
    run(&mut lc, &mut sys, &schedule_swap("hello", "v2", 10, 2)).unwrap();
    commit(&mut lc);
    assert!(run(&mut lc, &mut sys, &cancel_swap("hello", "vX")).is_err());
    let mut late = ctx(Origin::System, 10);
    assert!(run(&mut lc, &mut late, &cancel_swap("hello", "v2")).is_err());
    run(&mut lc, &mut sys, &cancel_swap("hello", "v2")).unwrap();
    commit(&mut lc);
    assert!(module_status(&lc)[0].pending.is_none());
    assert_eq!(module_status(&lc)[0].active_code_hash, hash(1));
}

// ---- admission (ScheduleRegister) -------------------------------------------

#[test]
fn admission_realizes_and_refuses_bad_inputs() {
    let mut lc = fresh();
    let mut gov = ctx_at_admission(Origin::Module("governance".into()), 0);
    run(&mut lc, &mut gov, &schedule_register("kanban", "v1", 10, 5)).unwrap();
    commit(&mut lc);
    assert!(module_status(&lc)[0].active_code_hash.is_empty());
    assert!(armed_at(&lc, 10).is_empty(), "not armed until ready");
    make_swap_ready(&mut lc, "kanban", "v1");
    assert_eq!(armed_at(&lc, 10)[0].code_hash, hash(5));
    let mut at = ctx(Origin::System, 10);
    run(&mut lc, &mut at, &advance()).unwrap();
    commit(&mut lc);
    assert_eq!(module_status(&lc)[0].active_code_hash, hash(5));
    assert!(module_status(&lc)[0].pending.is_none());

    // refuses an existing id, short lead, external origin, live host id.
    let mut lc = fresh();
    register_module(&mut lc, "hello", 1);
    let mut sys = ctx_at_admission(Origin::System, 0);
    assert!(run(&mut lc, &mut sys, &schedule_register("hello", "v1", 10, 5)).is_err());
    assert!(run(&mut lc, &mut sys, &schedule_register("kanban", "v1", MIN_SWAP_LEAD, 5)).is_err());
    let mut ext = ctx_at_admission(Origin::External(member(1)), 0);
    assert!(run(&mut lc, &mut ext, &schedule_register("kanban", "v1", 10, 5)).is_err());
    let mut live =
        ctx_at_admission(Origin::System, 0).with_module_root("valset", StateRoot::ZERO);
    assert!(run(&mut lc, &mut live, &schedule_register("valset", "v1", 10, 5)).is_err());
    // below the activation version.
    let mut old = ctx(Origin::System, 0);
    assert!(run(&mut lc, &mut old, &schedule_register("kanban", "v1", 10, 5)).is_err());
}

#[test]
fn cancelled_admission_removes_entry() {
    let mut lc = fresh();
    let mut sys = ctx_at_admission(Origin::System, 0);
    run(&mut lc, &mut sys, &schedule_register("kanban", "v1", 10, 5)).unwrap();
    commit(&mut lc);
    run(&mut lc, &mut sys, &cancel_swap("kanban", "v1")).unwrap();
    commit(&mut lc);
    assert!(module_status(&lc).is_empty());
}

// ============================================================================
// merged root + snapshot + shared Advance
// ============================================================================

#[test]
fn root_zero_fresh_then_each_half_moves_it() {
    let mut lc = fresh();
    assert_eq!(lc.root(), StateRoot::ZERO);
    // the upgrade half alone moves root off ZERO.
    let mut sys = ctx(Origin::System, 0);
    run(&mut lc, &mut sys, &schedule_upgrade("a", 10, 2)).unwrap();
    commit(&mut lc);
    let after_upgrade = lc.root();
    assert_ne!(after_upgrade, StateRoot::ZERO);
    // registering a module moves it again (both halves contribute to one root).
    register_module(&mut lc, "hello", 1);
    assert_ne!(lc.root(), after_upgrade);
}

#[test]
fn combined_snapshot_round_trips_both_halves() {
    let members = vec![member(1)];
    let mut lc = fresh();
    let mut sys = ctx(Origin::System, 0).with_members(members.clone());
    // populate BOTH halves: a pending upgrade with a signal, a module with an
    // active hash and a pending, ready swap.
    run(&mut lc, &mut sys, &schedule_upgrade("up", 10, 2)).unwrap();
    commit(&mut lc);
    let mut v = ctx(Origin::External(member(1)), 0).with_members(members.clone());
    run(&mut lc, &mut v, &upgrade_ready("up", 2)).unwrap();
    commit(&mut lc);
    register_module(&mut lc, "hello", 1);
    run(&mut lc, &mut sys, &schedule_swap("hello", "v2", 20, 2)).unwrap();
    commit(&mut lc);
    make_swap_ready(&mut lc, "hello", "v2");

    let bytes = lc.snapshot();
    let root = lc.root();
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    assert_eq!(StateRoot(digest), root, "sha256(snapshot) == root");
    let mut dst = fresh();
    dst.install(&bytes, root).expect("install round-trips");
    assert_eq!(dst.committed, lc.committed);
    assert_eq!(dst.root(), root);

    // tamper rejected, target untouched.
    let mut flipped = bytes.clone();
    flipped[0] ^= 0x01;
    let mut other = fresh();
    register_module(&mut other, "z", 7);
    let pre = other.root();
    assert!(other.install(&flipped, root).is_err());
    assert!(other.install(&bytes[..bytes.len() - 1], root).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(other.install(&trailing, root).is_err());
    assert_eq!(other.root(), pre, "failed install left target untouched");
}

/// ONE Advance reconciles BOTH halves in a single dispatch: it arms the pending
/// upgrade AND activates an armed code swap that both reach the boundary at H.
#[test]
fn one_advance_ticks_both_halves() {
    let members = vec![member(1)];
    let mut lc = fresh();
    let mut sys = ctx(Origin::System, 0).with_members(members.clone());
    run(&mut lc, &mut sys, &schedule_upgrade("up", 10, 2)).unwrap();
    commit(&mut lc);
    let mut v = ctx(Origin::External(member(1)), 0).with_members(members.clone());
    run(&mut lc, &mut v, &upgrade_ready("up", 2)).unwrap();
    commit(&mut lc);
    register_module(&mut lc, "hello", 1);
    run(&mut lc, &mut sys, &schedule_swap("hello", "v2", 10, 2)).unwrap();
    commit(&mut lc);
    make_swap_ready(&mut lc, "hello", "v2");

    let mut boundary = ctx(Origin::System, 10).with_members(members);
    run(&mut lc, &mut boundary, &advance()).unwrap();
    commit(&mut lc);
    assert_eq!(lc.committed.current_version, 2, "upgrade armed");
    assert!(lc.committed.pending_upgrade.is_none());
    assert_eq!(module_status(&lc)[0].active_code_hash, hash(2), "swap activated");
    assert!(module_status(&lc)[0].pending.is_none());
}
