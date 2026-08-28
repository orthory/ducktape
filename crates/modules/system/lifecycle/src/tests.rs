//! the lifecycle proof: the module-code path on one module, one root, one wire.

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

fn env_at(origin: Origin, height: u64) -> sdk::Env {
    sdk::Env {
        height,
        consensus_time: 0,
        origin,
        me: "lifecycle".into(),
    }
}

/// a ctx over `origin`/`height`, valset defaulting to a single member.
fn ctx(origin: Origin, height: u64) -> TestCtx {
    TestCtx::with_env(env_at(origin, height)).on_query("valset", validators(vec![member(1)]))
}

/// lifecycle-domain verb kept as a call-site method so the `.with_members`
/// sites read unchanged: it re-keys the valset reply.
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
    Lifecycle::new(
        "lifecycle",
        Box::new(sdk_testkit::MemStore::new()),
        "valset",
    )
}

/// the root of a store that never committed anything — the store-backed twin
/// of the old ZERO sentinel.
fn empty_root() -> StateRoot {
    fresh().root()
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

fn advance() -> Msg {
    msg(LifecycleMsg::Advance)
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
    let bytes = futures::executor::block_on(lc.query_with(
        &ctx(Origin::System, 0),
        &encode_query(&LifecycleQuery::ModuleStatus),
    ))
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
        run(
            &mut lc,
            &mut ext,
            &schedule_swap("hello", "replacement", 10, 2)
        ),
        Err(Error::Module(_))
    ));
    run(
        &mut lc,
        &mut gov,
        &schedule_swap("hello", "replacement", 10, 2),
    )
    .unwrap();
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
    assert!(
        run(
        &mut lc,
        &mut sys,
        &msg(LifecycleMsg::RegisterModule {
            module_id: "hello".into(),
            code_hash: hash(9)
        })
    )
        .is_err()
    );
    assert!(
        run(
        &mut lc,
        &mut sys,
        &msg(LifecycleMsg::RegisterModule {
            module_id: "other".into(),
            code_hash: vec![1, 2, 3]
        })
    )
        .is_err()
    );
}

#[test]
fn schedule_swap_validation() {
    let mut lc = fresh();
    register_module(&mut lc, "hello", 1);
    let mut sys = ctx(Origin::System, 0);
    // unregistered module.
    assert!(
        run(
            &mut lc,
            &mut sys,
            &schedule_swap("ghost", "replacement", 10, 2)
        )
        .is_err()
    );
    // min lead.
    assert!(
        run(
            &mut lc,
            &mut sys,
            &schedule_swap("hello", "replacement", 3, 2)
        )
        .is_err()
    );
    // no-op swap.
    assert!(
        run(
            &mut lc,
            &mut sys,
            &schedule_swap("hello", "replacement", 10, 1)
        )
        .is_err()
    );
    run(
        &mut lc,
        &mut sys,
        &schedule_swap("hello", "replacement", 10, 2),
    )
    .unwrap();
    commit(&mut lc);
    // second pending.
    assert!(run(&mut lc, &mut sys, &schedule_swap("hello", "other", 20, 3)).is_err());
}

#[test]
fn swap_advance_activates_at_height_and_frees_slot() {
    let mut lc = fresh();
    register_module(&mut lc, "hello", 1);
    let mut sys = ctx(Origin::System, 0);
    run(
        &mut lc,
        &mut sys,
        &schedule_swap("hello", "replacement", 10, 2),
    )
    .unwrap();
    commit(&mut lc);
    make_swap_ready(&mut lc, "hello", "replacement");

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
    run(
        &mut lc,
        &mut sys,
        &schedule_swap("hello", "replacement", 10, 2),
    )
    .unwrap();
    commit(&mut lc);
    // no readiness: never arms.
    assert!(armed_at(&lc, u64::MAX).is_empty());

    let two = vec![member(1), member(2)];
    let mut m1 = ctx(Origin::External(member(1)), 0).with_members(two.clone());
    run(&mut lc, &mut m1, &swap_ready("hello", "replacement")).unwrap();
    commit(&mut lc);
    assert!(!module_status(&lc)[0].pending.clone().unwrap().ready);
    assert!(armed_at(&lc, 10).is_empty());

    let mut m2 = ctx(Origin::External(member(2)), 0).with_members(two);
    run(&mut lc, &mut m2, &swap_ready("hello", "replacement")).unwrap();
    commit(&mut lc);
    assert!(module_status(&lc)[0].pending.clone().unwrap().ready);
    assert_eq!(armed_at(&lc, 10).len(), 1);
}

#[test]
fn swap_signal_gates_origin_and_identity() {
    let mut lc = fresh();
    register_module(&mut lc, "hello", 1);
    let mut sys = ctx(Origin::System, 0);
    run(
        &mut lc,
        &mut sys,
        &schedule_swap("hello", "replacement", 10, 2),
    )
    .unwrap();
    commit(&mut lc);
    assert!(run(&mut lc, &mut sys, &swap_ready("hello", "replacement")).is_err());
    let mut stranger = ctx(Origin::External(member(9)), 0);
    assert!(run(&mut lc, &mut stranger, &swap_ready("hello", "replacement")).is_err());
    let mut m1 = ctx(Origin::External(member(1)), 0);
    assert!(run(&mut lc, &mut m1, &swap_ready("hello", "vX")).is_err());
}

#[test]
fn swap_cancel_guards_and_clears() {
    let mut lc = fresh();
    register_module(&mut lc, "hello", 1);
    let mut sys = ctx(Origin::System, 0);
    run(
        &mut lc,
        &mut sys,
        &schedule_swap("hello", "replacement", 10, 2),
    )
    .unwrap();
    commit(&mut lc);
    assert!(run(&mut lc, &mut sys, &cancel_swap("hello", "vX")).is_err());
    let mut late = ctx(Origin::System, 10);
    assert!(run(&mut lc, &mut late, &cancel_swap("hello", "replacement")).is_err());
    run(&mut lc, &mut sys, &cancel_swap("hello", "replacement")).unwrap();
    commit(&mut lc);
    assert!(module_status(&lc)[0].pending.is_none());
    assert_eq!(module_status(&lc)[0].active_code_hash, hash(1));
}

// ---- admission (ScheduleRegister) -------------------------------------------

#[test]
fn admission_realizes_and_refuses_bad_inputs() {
    let mut lc = fresh();
    let mut gov = ctx(Origin::Module("governance".into()), 0);
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
    let mut sys = ctx(Origin::System, 0);
    assert!(run(&mut lc, &mut sys, &schedule_register("hello", "v1", 10, 5)).is_err());
    assert!(
        run(
            &mut lc,
            &mut sys,
            &schedule_register("kanban", "v1", MIN_SWAP_LEAD, 5)
        )
        .is_err()
    );
    let mut ext = ctx(Origin::External(member(1)), 0);
    assert!(run(&mut lc, &mut ext, &schedule_register("kanban", "v1", 10, 5)).is_err());
    let mut live = ctx(Origin::System, 0).with_module_root("valset", StateRoot::ZERO);
    assert!(
        run(
            &mut lc,
            &mut live,
            &schedule_register("valset", "v1", 10, 5)
        )
        .is_err()
    );
}

#[test]
fn cancelled_admission_removes_entry() {
    let mut lc = fresh();
    let mut sys = ctx(Origin::System, 0);
    run(&mut lc, &mut sys, &schedule_register("kanban", "v1", 10, 5)).unwrap();
    commit(&mut lc);
    run(&mut lc, &mut sys, &cancel_swap("kanban", "v1")).unwrap();
    commit(&mut lc);
    assert!(module_status(&lc).is_empty());
}

// ---- the activation history (code-at-height) --------------------------------

fn activation(height: u64, code: u8) -> Activation {
    Activation {
        height,
        code_hash: hash(code),
    }
}

/// a hand-built entry over `history` (`(height, code)` pairs, ascending).
fn entry(history: &[(u64, u8)], pending: Option<ScheduledSwap>) -> ModuleCode {
    ModuleCode {
        module_id: "hello".into(),
        active_code_hash: history.last().map(|(_, c)| hash(*c)).unwrap_or_default(),
        pending,
        history: history.iter().map(|(h, c)| activation(*h, *c)).collect(),
    }
}

#[test]
fn code_at_reads_the_armed_pending_then_the_history() {
    let e = entry(&[(10, 1), (50, 2)], None);
    assert_eq!(code_at(&e, 20), Some(hash(1).as_slice()));
    assert_eq!(code_at(&e, 50), Some(hash(2).as_slice()));
    assert_eq!(code_at(&e, 70), Some(hash(2).as_slice()));
    // before the first activation: the first code is the natural seat.
    assert_eq!(code_at(&e, 5), Some(hash(1).as_slice()));

    // a pending swap armed at `height` wins — the live pre-flip read.
    let armed = ScheduledSwap {
        name: "replacement".into(),
        activation_height: 50,
        code_hash: hash(2),
        readiness: vec![member(1)],
        ready: true,
    };
    let e = entry(&[(10, 1)], Some(armed.clone()));
    assert_eq!(code_at(&e, 50), Some(hash(2).as_slice()));
    assert_eq!(code_at(&e, 49), Some(hash(1).as_slice()));
    // an unready pending never arms, however high the height.
    let unready = ScheduledSwap {
        ready: false,
        ..armed
    };
    let e = entry(&[(10, 1)], Some(unready));
    assert_eq!(code_at(&e, 99), Some(hash(1).as_slice()));

    // registered, never activated: no code at all.
    assert_eq!(code_at(&entry(&[], None), 99), None);
}

#[test]
fn every_activation_is_appended_in_block_order() {
    let mut lc = fresh();
    let mut at7 = ctx(Origin::System, 7);
    run(
        &mut lc,
        &mut at7,
        &msg(LifecycleMsg::RegisterModule {
            module_id: "hello".into(),
            code_hash: hash(1),
        }),
    )
    .unwrap();
    commit(&mut lc);
    assert_eq!(module_status(&lc)[0].history, [activation(7, 1)]);

    run(
        &mut lc,
        &mut at7,
        &schedule_swap("hello", "replacement", 30, 2),
    )
    .unwrap();
    commit(&mut lc);
    make_swap_ready(&mut lc, "hello", "replacement");
    assert_eq!(
        module_status(&lc)[0].history,
        [activation(7, 1)],
        "scheduling records nothing: only a flip is an activation"
    );
    let mut at30 = ctx(Origin::System, 30);
    run(&mut lc, &mut at30, &advance()).unwrap();
    commit(&mut lc);
    assert_eq!(
        module_status(&lc)[0].history,
        [activation(7, 1), activation(30, 2)]
    );

    // an admission has no history until its boundary flips it.
    let mut lc = fresh();
    let mut sys = ctx(Origin::System, 0);
    run(&mut lc, &mut sys, &schedule_register("kanban", "v1", 10, 5)).unwrap();
    commit(&mut lc);
    assert!(module_status(&lc)[0].history.is_empty());
    make_swap_ready(&mut lc, "kanban", "v1");
    let mut at10 = ctx(Origin::System, 10);
    run(&mut lc, &mut at10, &advance()).unwrap();
    commit(&mut lc);
    assert_eq!(module_status(&lc)[0].history, [activation(10, 5)]);

    // a genesis seed is the activation at block zero.
    let mut lc = fresh();
    futures::executor::block_on(async {
        lc.seed("hello", hash(1)).await.unwrap();
        lc.finish_seed().await.unwrap();
    });
    assert_eq!(module_status(&lc)[0].history, [activation(0, 1)]);
}

// ============================================================================
// root + snapshot
// ============================================================================

#[test]
fn root_empty_fresh_then_state_moves_it() {
    let mut lc = fresh();
    assert_eq!(lc.root(), empty_root());
    register_module(&mut lc, "hello", 1);
    let after_register = lc.root();
    assert_ne!(after_register, empty_root());
    let mut sys = ctx(Origin::System, 0);
    run(
        &mut lc,
        &mut sys,
        &schedule_swap("hello", "replacement", 10, 2),
    )
    .unwrap();
    commit(&mut lc);
    assert_ne!(lc.root(), after_register);
}

#[test]
fn genesis_seed_publishes_once_and_reseeding_is_a_no_op() {
    let mut lc = fresh();
    futures::executor::block_on(async {
        lc.seed("hello", hash(1)).await.unwrap();
        lc.seed("directory", hash(2)).await.unwrap();
        lc.finish_seed().await.unwrap();
    });
    let seeded = lc.root();
    assert_ne!(seeded, empty_root(), "the seed set is committed state");
    assert_eq!(module_status(&lc).len(), 2);

    // a reopened workspace re-entering the genesis path re-seeds — the
    // idempotence gate must leave the store byte-untouched.
    futures::executor::block_on(async {
        lc.seed("hello", hash(9)).await.unwrap();
        lc.finish_seed().await.unwrap();
    });
    assert_eq!(
        lc.root(),
        seeded,
        "re-seeding an initialized store is a no-op"
    );
    let status = module_status(&lc);
    assert_eq!(
        status
            .iter()
            .find(|m| m.module_id == "hello")
            .unwrap()
            .active_code_hash,
        hash(1),
        "the original seed survived the re-entry"
    );
}
