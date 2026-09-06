//! the dispatch plane's behaviour suite over an in-memory store.
//!
//! these tests assert BEHAVIOUR — the recipe admin surface, the saga callback
//! correlation gates, the never-pop-stack delivery sweep, retention — so the
//! in-memory [`MemStore`] stands in for qmdb. the cross-node round trip over the
//! REAL store is `tests/sync_round_trip.rs`, and the wasm-vs-native root
//! continuity proof is the host crate's `wasm_dispatch_parity`.

use super::*;
use futures::executor::block_on;
use saga::{decode_msg as saga_decode_msg, encode_callback};
use sdk::Env;
use sdk_testkit::{MemStore, TestCtx};

// dispatch's execute reads only env; the shared TestCtx captures emitted
// msgs/events (read via msgs()/events()). module_root is never consulted.
fn mk_ctx(height: u64, origin: Origin) -> TestCtx {
    TestCtx::with_env(Env {
        height,
        consensus_time: height,
        origin,
        me: "dispatch".into(),
    })
}

/// build the module the way a host does: concrete store first, injected as
/// `Box<dyn MerkleStore>`.
fn module() -> DispatchModule {
    DispatchModule::new("dispatch", "saga", Box::new(MemStore::new()))
}
fn exec(m: &mut DispatchModule, ctx: &mut TestCtx, payload: &DispatchMsg) -> Result<(), Error> {
    let msg = Msg {
        target: "dispatch".into(),
        payload: crate::encode_msg(payload),
    };
    block_on(m.execute(ctx, &msg))
}
fn commit(m: &mut DispatchModule) {
    block_on(m.commit_block()).unwrap();
}
fn register(kind: OutputContract, routing: Routing) -> DispatchMsg {
    DispatchMsg::RegisterRecipe {
        recipe_id: "summarize".into(),
        description: "test recipe".into(),
        capability: "alpha".into(),
        routing,
        output_contract: kind,
        max_attempts: 2,
        deadline_views: Some(100),
        lease_views: None,
    }
}
fn owner() -> Origin {
    Origin::External(b"owner".to_vec())
}
fn dispatch_op(id: &str, payload: &[u8]) -> DispatchMsg {
    DispatchMsg::Dispatch {
        dispatch_id: id.into(),
        recipe_id: "summarize".into(),
        payload: payload.to_vec(),
        demands: BTreeMap::new(),
        admission: AdmissionPolicy::Queue,
    }
}
/// run register (as the external owner) + dispatch (as module "caller"),
/// committed — the shared preamble of the callback/delivery tests.
fn registered_and_dispatched(m: &mut DispatchModule, kind: OutputContract) -> String {
    let mut ctx = mk_ctx(0, owner());
    exec(m, &mut ctx, &register(kind, Routing::Rendezvous)).unwrap();
    let mut ctx = mk_ctx(5, Origin::Module("caller".into()));
    exec(m, &mut ctx, &dispatch_op("d1", b"input")).unwrap();
    commit(m);
    dispatch_key("caller", "d1")
}
fn callback_for(
    m: &mut DispatchModule,
    at: u64,
    key: &str,
    outcome: SagaOutcome,
) -> Result<(), Error> {
    let mut ctx = mk_ctx(at, Origin::Module("saga".into()));
    let msg = Msg {
        target: "dispatch".into(),
        payload: encode_callback(&SagaCallback {
            saga_id: saga_id_for(key),
            payload: key.as_bytes().to_vec(),
            outcome,
        }),
    };
    block_on(m.execute(&mut ctx, &msg))
}
fn get_dispatch(m: &DispatchModule, key: &str) -> Option<DispatchView> {
    // the tests address dispatches by their composite state key; split it
    // back into the wire query's (receiver, local id) coordinates.
    let (receiver, dispatch_id) = key.split_once(SEP).expect("composite key");
    let reply = block_on(m.query(&crate::encode_query(&DispatchQuery::Dispatch {
        receiver: receiver.into(),
        dispatch_id: dispatch_id.into(),
    })))
    .unwrap();
    match crate::decode_reply(&reply).unwrap() {
        DispatchReply::Dispatch(v) => v,
        other => panic!("expected Dispatch reply, got {other:?}"),
    }
}
fn pending_deliveries(m: &DispatchModule) -> u64 {
    let reply = block_on(m.query(&crate::encode_query(&DispatchQuery::PendingDeliveries))).unwrap();
    match crate::decode_reply(&reply).unwrap() {
        DispatchReply::PendingDeliveries(n) => n,
        other => panic!("expected PendingDeliveries reply, got {other:?}"),
    }
}
fn recipe(m: &DispatchModule, recipe_id: &str) -> Option<Recipe> {
    let reply = block_on(m.query(&crate::encode_query(&DispatchQuery::Recipe {
        recipe_id: recipe_id.into(),
    })))
    .unwrap();
    match crate::decode_reply(&reply).unwrap() {
        DispatchReply::Recipe(r) => r,
        other => panic!("expected Recipe reply, got {other:?}"),
    }
}
/// the committed byte size of one dispatch record — what the retention pin
/// measures now that the state IS the store.
fn record_bytes(m: &DispatchModule, key: &str) -> usize {
    block_on(m.staged.get_committed(&dispatch_key_of(key)))
        .unwrap()
        .expect("the record survives")
        .len()
}

#[test]
fn recipe_registration_validates_and_gates_mutation_by_owner() {
    let mut m = module();
    let mut ctx = mk_ctx(0, owner());
    exec(
        &mut m,
        &mut ctx,
        &register(OutputContract::Text, Routing::Rendezvous),
    )
    .unwrap();
    // duplicate id is an error.
    let err = exec(
        &mut m,
        &mut ctx,
        &register(OutputContract::Text, Routing::Rendezvous),
    )
    .unwrap_err();
    assert!(err.to_string().contains("already exists"), "got {err}");

    // shape rules: bad tag, empty pinned key, zero attempts.
    for (bad, needle) in [
        (
            DispatchMsg::RegisterRecipe {
                recipe_id: "r2".into(),
                description: String::new(),
                capability: "NOT A TAG".into(),
                routing: Routing::Rendezvous,
                output_contract: OutputContract::Text,
                max_attempts: 1,
                deadline_views: None,
                lease_views: None,
            },
            "invalid characters",
        ),
        (
            DispatchMsg::RegisterRecipe {
                recipe_id: "r2".into(),
                description: String::new(),
                capability: "alpha".into(),
                routing: Routing::Pinned(Vec::new()),
                output_contract: OutputContract::Text,
                max_attempts: 1,
                deadline_views: None,
                lease_views: None,
            },
            "Pinned key",
        ),
        (
            DispatchMsg::RegisterRecipe {
                recipe_id: "r2".into(),
                description: String::new(),
                capability: "alpha".into(),
                routing: Routing::Rendezvous,
                output_contract: OutputContract::Text,
                max_attempts: 0,
                deadline_views: None,
                lease_views: None,
            },
            "max_attempts",
        ),
    ] {
        let err = exec(&mut m, &mut ctx, &bad).unwrap_err();
        assert!(err.to_string().contains(needle), "wanted {needle} in {err}");
    }

    // a foreign origin cannot update or remove.
    let mut foreign = mk_ctx(0, Origin::External(b"other".to_vec()));
    let err = exec(
        &mut m,
        &mut foreign,
        &DispatchMsg::RemoveRecipe {
            recipe_id: "summarize".into(),
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("not owned"), "got {err}");

    // the owner can update; the update is validated too.
    let err = exec(
        &mut m,
        &mut ctx,
        &DispatchMsg::UpdateRecipe {
            recipe_id: "summarize".into(),
            description: None,
            capability: Some("ALSO NOT A TAG".into()),
            routing: None,
            output_contract: None,
            max_attempts: None,
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("invalid characters"), "got {err}");
    exec(
        &mut m,
        &mut ctx,
        &DispatchMsg::UpdateRecipe {
            recipe_id: "summarize".into(),
            description: None,
            capability: Some("beta".into()),
            routing: None,
            output_contract: None,
            max_attempts: Some(3),
        },
    )
    .unwrap();
    commit(&mut m);
    let committed = recipe(&m, "summarize").expect("recipe committed");
    assert_eq!(committed.capability, "beta");
    assert_eq!(committed.max_attempts, 3);
}

#[test]
fn agent_namespace_is_reserved_for_the_runs_module_origin() {
    let mut m = module();

    // an External account can never claim an `agent/` recipe id — squatting
    // it would permanently block that agent's own RegisterAgent hook.
    let mut mallory = mk_ctx(0, Origin::External(b"mallory".to_vec()));
    let err = exec(
        &mut m,
        &mut mallory,
        &DispatchMsg::RegisterRecipe {
            recipe_id: "agent/bot".into(),
            description: String::new(),
            capability: "alpha".into(),
            routing: Routing::Rendezvous,
            output_contract: OutputContract::Text,
            max_attempts: 1,
            deadline_views: None,
            lease_views: None,
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("reserved"), "got {err}");

    // the runs module's own module-origin registration succeeds — this is
    // the exact RegisterRecipe the registry hook emits as a follow-up of
    // agent_intake's RegisterAgent.
    let mut runs = mk_ctx(0, Origin::Module("runs".into()));
    exec(
        &mut m,
        &mut runs,
        &DispatchMsg::RegisterRecipe {
            recipe_id: "agent/bot".into(),
            description: "runs for agent bot".into(),
            capability: "alpha".into(),
            routing: Routing::Rendezvous,
            output_contract: OutputContract::Text,
            max_attempts: 1,
            deadline_views: None,
            lease_views: None,
        },
    )
    .unwrap();
    commit(&mut m);
    assert!(recipe(&m, "agent/bot").is_some());

    // a foreign module (never runs) still cannot remove it.
    let mut other = mk_ctx(0, Origin::Module("other".into()));
    let err = exec(
        &mut m,
        &mut other,
        &DispatchMsg::RemoveRecipe {
            recipe_id: "agent/bot".into(),
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("not owned"), "got {err}");

    // runs (the reserved owner) can remove its own reserved recipe.
    exec(
        &mut m,
        &mut runs,
        &DispatchMsg::RemoveRecipe {
            recipe_id: "agent/bot".into(),
        },
    )
    .unwrap();
    commit(&mut m);
    assert!(recipe(&m, "agent/bot").is_none());
}

#[test]
fn removing_every_recipe_returns_the_root_to_its_never_registered_value() {
    // a recipe's whole footprint is its own `r/{id}` record, so removing every
    // recipe must leave the plane hashing exactly like one that never
    // registered any — no residue key surviving the removals.
    let mut m = module();
    let empty_root = m.root();
    let mut ctx = mk_ctx(0, owner());
    for recipe_id in ["zeta", "alpha"] {
        exec(
            &mut m,
            &mut ctx,
            &DispatchMsg::RegisterRecipe {
                recipe_id: recipe_id.into(),
                description: String::new(),
                capability: "alpha".into(),
                routing: Routing::Rendezvous,
                output_contract: OutputContract::Text,
                max_attempts: 1,
                deadline_views: None,
                lease_views: None,
            },
        )
        .unwrap();
    }
    commit(&mut m);
    assert!(recipe(&m, "alpha").is_some());
    assert!(recipe(&m, "zeta").is_some());

    for recipe_id in ["alpha", "zeta"] {
        exec(
            &mut m,
            &mut ctx,
            &DispatchMsg::RemoveRecipe {
                recipe_id: recipe_id.into(),
            },
        )
        .unwrap();
    }
    commit(&mut m);
    assert!(recipe(&m, "alpha").is_none());
    assert!(recipe(&m, "zeta").is_none());
    assert_eq!(
        m.root(),
        empty_root,
        "a removed recipe must leave no residue key behind"
    );
}

#[test]
fn a_routing_pin_over_saga_s_assignee_cap_is_refused_at_registration() {
    // saga refuses a `pinned_assignee` over MAX_ASSIGNEE_BYTES at trigger
    // time, so a recipe admitted with a bigger pin would register fine and
    // then fail EVERY dispatch under it. registration is where the pin is
    // admitted, so registration is where it is capped.
    let mut m = module();
    let mut ctx = mk_ctx(0, owner());
    let err = exec(
        &mut m,
        &mut ctx,
        &DispatchMsg::RegisterRecipe {
            recipe_id: "huge".into(),
            description: String::new(),
            capability: "alpha".into(),
            routing: Routing::Pinned(vec![7u8; 300]),
            output_contract: OutputContract::Text,
            max_attempts: 1,
            deadline_views: None,
            lease_views: None,
        },
    )
    .unwrap_err();
    assert!(
        err.to_string()
            .contains(&format!("the cap is {MAX_ASSIGNEE_BYTES}")),
        "the refusal must name the cap; got {err}"
    );
    // and the refusal left NOTHING staged.
    commit(&mut m);
    assert!(recipe(&m, "huge").is_none());
}

#[test]
fn a_pin_at_the_cap_registers_and_its_trigger_carries_it() {
    // the boundary the other side of the refusal: a pin exactly at the cap is
    // admitted, and the dispatch it drives emits a trigger saga's own gate
    // accepts — the two caps are the same number, so this can never be
    // register-ok-then-dispatch-refused.
    let mut m = module();
    let pin = vec![7u8; MAX_ASSIGNEE_BYTES];
    let mut ctx = mk_ctx(0, owner());
    exec(
        &mut m,
        &mut ctx,
        &register(OutputContract::Text, Routing::Pinned(pin.clone())),
    )
    .unwrap();
    commit(&mut m);

    let mut caller = mk_ctx(5, Origin::Module("caller".into()));
    exec(&mut m, &mut caller, &dispatch_op("d1", b"input")).unwrap();
    assert_eq!(caller.msgs().len(), 1);
    let SagaMsg::Trigger {
        pinned_assignee, ..
    } = saga_decode_msg(&caller.msgs()[0].payload).unwrap()
    else {
        panic!("expected a trigger");
    };
    assert_eq!(pinned_assignee, Some(pin));
}

#[test]
fn dispatch_stages_trigger_with_recipe_routing_and_dedups_per_receiver() {
    let mut m = module();
    let mut ctx = mk_ctx(0, owner());
    exec(
        &mut m,
        &mut ctx,
        &register(OutputContract::Json, Routing::Pinned(vec![7u8; 32])),
    )
    .unwrap();
    commit(&mut m);

    // external dispatch is refused.
    let mut external = mk_ctx(0, owner());
    let err = exec(&mut m, &mut external, &dispatch_op("d1", b"x")).unwrap_err();
    assert!(err.to_string().contains("module-origin only"), "got {err}");

    // an unknown recipe is an error; an oversized payload is an error.
    let mut caller = mk_ctx(5, Origin::Module("caller".into()));
    let err = exec(
        &mut m,
        &mut caller,
        &DispatchMsg::Dispatch {
            dispatch_id: "d1".into(),
            recipe_id: "nope".into(),
            payload: b"x".to_vec(),
            demands: BTreeMap::new(),
            admission: AdmissionPolicy::Queue,
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("unknown recipe"), "got {err}");
    let err = exec(
        &mut m,
        &mut caller,
        &dispatch_op("d1", &vec![0u8; MAX_PAYLOAD_BYTES + 1]),
    )
    .unwrap_err();
    assert!(err.to_string().contains("payload is"), "got {err}");

    // the real dispatch stages exactly one trigger carrying the recipe's
    // capability, routing pin, deadline, and the work-spec payload.
    exec(&mut m, &mut caller, &dispatch_op("d1", b"input")).unwrap();
    assert_eq!(caller.msgs().len(), 1);
    assert_eq!(caller.msgs()[0].target, "saga");
    let SagaMsg::Trigger {
        saga_id,
        spec,
        reply_to,
        reply_payload,
        deadline,
        max_attempts,
        capability,
        pinned_assignee,
        ..
    } = saga_decode_msg(&caller.msgs()[0].payload).unwrap()
    else {
        panic!("expected a trigger");
    };
    let key = dispatch_key("caller", "d1");
    assert_eq!(saga_id, saga_id_for(&key));
    assert_eq!(reply_to.as_deref(), Some("dispatch"));
    assert_eq!(reply_payload, key.clone().into_bytes());
    assert_eq!(deadline, Some(5 + 100));
    assert_eq!(max_attempts, 2);
    assert_eq!(capability.as_deref(), Some("alpha"));
    assert_eq!(pinned_assignee, Some(vec![7u8; 32]));
    let work = crate::decode_work_spec(&spec).unwrap();
    assert_eq!(work.dispatch_id, "d1");
    assert_eq!(work.capability, "alpha");
    assert_eq!(work.payload, b"input");

    // a duplicate under the same receiver is a deterministic no-op...
    exec(&mut m, &mut caller, &dispatch_op("d1", b"other")).unwrap();
    assert_eq!(caller.msgs().len(), 1, "no second trigger");
    // ...but another receiver's identical id is a distinct dispatch.
    let mut other = mk_ctx(5, Origin::Module("other".into()));
    exec(&mut m, &mut other, &dispatch_op("d1", b"input")).unwrap();
    assert_eq!(other.msgs().len(), 1);
    commit(&mut m);
    assert!(get_dispatch(&m, &key).is_some());
    assert!(get_dispatch(&m, &dispatch_key("other", "d1")).is_some());
}

#[test]
fn dispatch_demands_reach_both_the_trigger_and_the_work_spec() {
    let mut m = module();
    let mut ctx = mk_ctx(0, owner());
    exec(
        &mut m,
        &mut ctx,
        &register(OutputContract::Text, Routing::Rendezvous),
    )
    .unwrap();
    commit(&mut m);

    let demands = BTreeMap::from([("cores".to_string(), 4u64)]);
    let mut caller = mk_ctx(5, Origin::Module("caller".into()));
    exec(
        &mut m,
        &mut caller,
        &DispatchMsg::Dispatch {
            dispatch_id: "d1".into(),
            recipe_id: "summarize".into(),
            payload: b"input".to_vec(),
            demands: demands.clone(),
            admission: AdmissionPolicy::FailFast,
        },
    )
    .unwrap();
    assert_eq!(caller.msgs().len(), 1);
    let SagaMsg::Trigger {
        spec,
        demands: trigger_demands,
        ..
    } = saga_decode_msg(&caller.msgs()[0].payload).unwrap()
    else {
        panic!("expected a trigger");
    };
    // ONE source: the trigger's demands and the work spec's demands agree
    // exactly with what was dispatched — no drift possible.
    assert_eq!(trigger_demands, demands);
    let work = crate::decode_work_spec(&spec).unwrap();
    assert_eq!(work.demands, demands);
    assert_eq!(work.admission, AdmissionPolicy::FailFast);
}

#[test]
fn work_spec_without_demands_field_is_rejected() {
    // FLAG DAY: demands is required — a spec that omits the key fails to
    // decode rather than silently defaulting to a demandless job.
    let no_demands =
        br#"{"kind":"dispatch-work-v1","dispatch_id":"d","capability":"c","payload":[]}"#;
    assert!(crate::decode_work_spec(no_demands).is_err());
}

#[test]
fn callbacks_judge_contracts_and_enqueue_deliveries() {
    // Json contract, JSON result: Ok flows to the mailbox.
    let mut m = module();
    let key = registered_and_dispatched(&mut m, OutputContract::Json);
    callback_for(
        &mut m,
        9,
        &key,
        SagaOutcome::Done(br#"{"ok":true}"#.to_vec()),
    )
    .unwrap();
    commit(&mut m);
    let view = get_dispatch(&m, &key).unwrap();
    assert_eq!(view.status, DispatchStatus::AwaitingDelivery);
    assert_eq!(view.outcome, Some(Ok(br#"{"ok":true}"#.to_vec())));
    assert_eq!(pending_deliveries(&m), 1);

    // Json contract, non-JSON result: the VIOLATION is the outcome.
    let mut m = module();
    let key = registered_and_dispatched(&mut m, OutputContract::Json);
    callback_for(&mut m, 9, &key, SagaOutcome::Done(b"not json".to_vec())).unwrap();
    commit(&mut m);
    let view = get_dispatch(&m, &key).unwrap();
    match view.outcome {
        Some(Err(e)) => assert!(e.contains("output contract violation"), "got {e}"),
        other => panic!("expected a contract violation, got {other:?}"),
    }
    assert_eq!(pending_deliveries(&m), 1, "violations are delivered too");

    // saga failure maps to the Err outcome verbatim.
    let mut m = module();
    let key = registered_and_dispatched(&mut m, OutputContract::Text);
    callback_for(&mut m, 9, &key, SagaOutcome::Failed("provider died".into())).unwrap();
    commit(&mut m);
    assert_eq!(
        get_dispatch(&m, &key).unwrap().outcome,
        Some(Err("provider died".into()))
    );

    // correlation gates: unknown key and stale saga id are no-ops.
    let mut m = module();
    let key = registered_and_dispatched(&mut m, OutputContract::Text);
    let before = m.root();
    callback_for(
        &mut m,
        9,
        "caller\x1fnope",
        SagaOutcome::Done(b"x".to_vec()),
    )
    .unwrap();
    let mut ctx = mk_ctx(0, Origin::Module("saga".into()));
    let msg = Msg {
        target: "dispatch".into(),
        payload: encode_callback(&SagaCallback {
            saga_id: "some-other-saga".into(),
            payload: key.as_bytes().to_vec(),
            outcome: SagaOutcome::Done(b"x".to_vec()),
        }),
    };
    block_on(m.execute(&mut ctx, &msg)).unwrap();
    commit(&mut m);
    assert_eq!(m.root(), before, "mismatched callbacks stage nothing");
}

#[test]
fn deliver_pending_is_system_only_bounded_and_fifo() {
    let mut m = module();
    let mut ctx = mk_ctx(0, owner());
    exec(
        &mut m,
        &mut ctx,
        &register(OutputContract::Text, Routing::Rendezvous),
    )
    .unwrap();
    // enqueue MAX + 1 outcomes across two receivers.
    for i in 0..=MAX_DELIVERIES_PER_BLOCK {
        let receiver = if i % 2 == 0 { "even" } else { "odd" };
        let mut caller = mk_ctx(5, Origin::Module(receiver.into()));
        exec(
            &mut m,
            &mut caller,
            &dispatch_op(&format!("d{i:03}"), b"in"),
        )
        .unwrap();
    }
    commit(&mut m);
    for i in 0..=MAX_DELIVERIES_PER_BLOCK {
        let receiver = if i % 2 == 0 { "even" } else { "odd" };
        let key = dispatch_key(receiver, &format!("d{i:03}"));
        callback_for(
            &mut m,
            9,
            &key,
            SagaOutcome::Done(format!("r{i}").into_bytes()),
        )
        .unwrap();
    }
    commit(&mut m);
    assert_eq!(
        pending_deliveries(&m) as usize,
        MAX_DELIVERIES_PER_BLOCK + 1
    );

    // non-System origins cannot force a delivery sweep.
    let mut foreign = mk_ctx(0, owner());
    let err = exec(&mut m, &mut foreign, &DispatchMsg::DeliverPending {}).unwrap_err();
    assert!(err.to_string().contains("System-origin"), "got {err}");

    // the System sweep drains FIFO, bounded per block.
    let mut sys = mk_ctx(9, Origin::System);
    exec(&mut m, &mut sys, &DispatchMsg::DeliverPending {}).unwrap();
    commit(&mut m);
    assert_eq!(sys.msgs().len(), MAX_DELIVERIES_PER_BLOCK);
    // FIFO: the first emitted event is the first enqueued dispatch.
    let first = crate::decode_result_event(&sys.msgs()[0].payload).unwrap();
    assert_eq!(first.dispatch_id, "d000");
    assert_eq!(first.outcome, Ok(b"r0".to_vec()));
    assert_eq!(sys.msgs()[0].target, "even");
    assert_eq!(pending_deliveries(&m), 1, "the remainder stays pending");
    assert_eq!(
        get_dispatch(&m, &dispatch_key("even", "d000"))
            .unwrap()
            .status,
        DispatchStatus::Delivered
    );

    // the next sweep drains the remainder.
    let mut sys = mk_ctx(10, Origin::System);
    exec(&mut m, &mut sys, &DispatchMsg::DeliverPending {}).unwrap();
    commit(&mut m);
    assert_eq!(sys.msgs().len(), 1);
    assert_eq!(pending_deliveries(&m), 0);
}

#[test]
fn admission_policy_does_not_change_committed_state_or_root() {
    fn dispatched(admission: AdmissionPolicy) -> DispatchModule {
        let mut m = module();
        let mut owner_ctx = mk_ctx(0, owner());
        exec(
            &mut m,
            &mut owner_ctx,
            &register(OutputContract::Json, Routing::Rendezvous),
        )
        .unwrap();
        commit(&mut m);
        let mut caller = mk_ctx(5, Origin::Module("caller".into()));
        exec(
            &mut m,
            &mut caller,
            &DispatchMsg::Dispatch {
                dispatch_id: "d1".into(),
                recipe_id: "summarize".into(),
                payload: b"input".to_vec(),
                demands: BTreeMap::new(),
                admission,
            },
        )
        .unwrap();
        commit(&mut m);
        m
    }

    let queue = dispatched(AdmissionPolicy::Queue);
    let fail_fast = dispatched(AdmissionPolicy::FailFast);
    // admission is host-local: it rides the work spec, never a record.
    assert_eq!(queue.root(), fail_fast.root());
}

#[test]
fn abort_discards_every_staged_write() {
    let mut m = module();
    let before = m.root();
    let mut ctx = mk_ctx(0, owner());
    exec(
        &mut m,
        &mut ctx,
        &register(OutputContract::Text, Routing::Rendezvous),
    )
    .unwrap();
    let mut caller = mk_ctx(0, Origin::Module("caller".into()));
    exec(&mut m, &mut caller, &dispatch_op("d1", b"in")).unwrap();
    block_on(m.abort_block()).unwrap();
    assert_eq!(m.root(), before, "aborted block leaves no trace");
    assert_eq!(pending_deliveries(&m), 0);
}

#[test]
fn cancel_is_receiver_scoped_and_idempotent() {
    let mut m = module();
    let key = registered_and_dispatched(&mut m, OutputContract::Text);
    let cancel = DispatchMsg::CancelDispatch {
        dispatch_id: "d1".into(),
    };

    // an external submitter has no cancel surface.
    let mut ctx = mk_ctx(0, owner());
    assert!(exec(&mut m, &mut ctx, &cancel).is_err());

    // a foreign module's cancel lands in its OWN receiver namespace —
    // an unknown key, a deterministic no-op.
    let mut ctx = mk_ctx(0, Origin::Module("other".into()));
    exec(&mut m, &mut ctx, &cancel).unwrap();
    assert!(ctx.msgs().is_empty());

    // the receiver's cancel tells exactly the dispatch's own saga.
    let mut ctx = mk_ctx(0, Origin::Module("caller".into()));
    exec(&mut m, &mut ctx, &cancel).unwrap();
    assert_eq!(ctx.msgs().len(), 1);
    assert_eq!(ctx.msgs()[0].target, "saga");
    match saga_decode_msg(&ctx.msgs()[0].payload).unwrap() {
        SagaMsg::Cancel { saga_id } => assert_eq!(saga_id, saga_id_for(&key)),
        other => panic!("expected a saga Cancel, got {other:?}"),
    }

    // once the Cancelled callback lands, the result flows the NORMAL
    // path (Err in the mailbox) and a repeat cancel is a no-op.
    callback_for(&mut m, 9, &key, SagaOutcome::Cancelled).unwrap();
    commit(&mut m);
    assert_eq!(pending_deliveries(&m), 1);
    let mut ctx = mk_ctx(0, Origin::Module("caller".into()));
    exec(&mut m, &mut ctx, &cancel).unwrap();
    assert!(ctx.msgs().is_empty());
}

#[test]
fn an_undecodable_saga_callback_is_swallowed_not_an_abort() {
    // the callback-poison rule: the callback intake runs inside a
    // finalized block, so a payload that fails to decode must be a
    // staged no-op plus a diagnostic event — an Err would abort the
    // block, which replays as a no-op and re-aborts forever.
    let mut m = module();
    let key = registered_and_dispatched(&mut m, OutputContract::Text);
    let before = m.root();

    let mut ctx = mk_ctx(0, Origin::Module("saga".into()));
    let msg = Msg {
        target: "dispatch".into(),
        payload: b"not a callback".to_vec(),
    };
    block_on(m.execute(&mut ctx, &msg)).expect("the poisoned callback must not abort");
    assert_eq!(ctx.events().len(), 1, "the swallow left a diagnostic event");
    assert!(
        String::from_utf8_lossy(&ctx.events()[0].payload).contains("undecodable"),
        "the event names the drop"
    );

    // the block applies and stages nothing: the dispatch still awaits its
    // result and the root is unmoved.
    commit(&mut m);
    assert_eq!(m.root(), before, "nothing was staged");
    assert!(matches!(
        get_dispatch(&m, &key).unwrap().status,
        DispatchStatus::AwaitingResult { .. }
    ));
}

#[test]
fn an_orphaned_mailbox_entry_is_dropped_not_an_abort() {
    // the never-pop-stack rule's failure mode: a committed non-empty
    // mailbox is re-injected every block, so an entry the sweep cannot
    // deliver must be dropped (with an event), never an Err — an abort
    // here would poison every future block.
    let mut m = module();
    // an orphan cannot be built through the module's own transitions;
    // plant its records directly (same crate, test-only).
    m.staged.stage(mailbox_key(0), b"ghost\x1fentry".to_vec());
    stage_mailbox(&mut m.staged, Mailbox { head: 0, next: 1 });
    commit(&mut m);
    assert_eq!(pending_deliveries(&m), 1);

    let mut sys = mk_ctx(0, Origin::System);
    exec(&mut m, &mut sys, &DispatchMsg::DeliverPending {})
        .expect("the orphan must not abort the delivery block");
    assert!(sys.msgs().is_empty(), "no delivery was invented for it");
    assert_eq!(sys.events().len(), 1, "the drop left a diagnostic event");
    assert!(
        String::from_utf8_lossy(&sys.events()[0].payload).contains("orphaned"),
        "the event names the orphan"
    );

    // the block applies and the mailbox drains — no re-injection loop.
    commit(&mut m);
    assert_eq!(pending_deliveries(&m), 0, "the orphan seq was dropped");
}

#[test]
fn reassign_is_receiver_scoped_and_carries_the_expected_attempt() {
    let mut m = module();
    let key = registered_and_dispatched(&mut m, OutputContract::Text);
    let reassign = DispatchMsg::ReassignDispatch {
        dispatch_id: "d1".into(),
        attempt: 2,
    };
    let mut ctx = mk_ctx(0, Origin::Module("caller".into()));
    exec(&mut m, &mut ctx, &reassign).unwrap();
    assert_eq!(ctx.msgs().len(), 1);
    assert!(matches!(
        saga_decode_msg(&ctx.msgs()[0].payload).unwrap(),
        SagaMsg::Reassign { saga_id, attempt: 2 } if saga_id == saga_id_for(&key)
    ));
}

// ---- retention ---------------------------------------------------------

/// drive one dispatch all the way from `Dispatch` to `Delivered`, one
/// block per transition, with an `outcome_bytes`-sized result.
fn full_round(m: &mut DispatchModule, i: usize, outcome_bytes: usize) {
    let height = (i as u64 + 1) * 4;
    let dispatch_id = format!("d{i:04}");
    let key = dispatch_key("caller", &dispatch_id);

    let mut ctx = mk_ctx(height, Origin::Module("caller".into()));
    exec(m, &mut ctx, &dispatch_op(&dispatch_id, b"input")).unwrap();
    commit(m);

    callback_for(
        m,
        height + 1,
        &key,
        SagaOutcome::Done(vec![b'x'; outcome_bytes]),
    )
    .unwrap();
    commit(m);

    let mut ctx = mk_ctx(height + 2, Origin::System);
    exec(m, &mut ctx, &DispatchMsg::DeliverPending {}).unwrap();
    commit(m);
}

#[test]
fn delivery_hands_the_outcome_over_and_drops_this_module_s_copy() {
    let mut m = module();
    let mut ctx = mk_ctx(0, owner());
    exec(
        &mut m,
        &mut ctx,
        &register(OutputContract::Text, Routing::Rendezvous),
    )
    .unwrap();
    commit(&mut m);

    // the pre-delivery record still carries the bytes — the mailbox sweep
    // is what needs them.
    let key = dispatch_key("caller", "d0000");
    let mut ctx = mk_ctx(4, Origin::Module("caller".into()));
    exec(&mut m, &mut ctx, &dispatch_op("d0000", b"input")).unwrap();
    commit(&mut m);
    callback_for(&mut m, 9, &key, SagaOutcome::Done(b"result".to_vec())).unwrap();
    commit(&mut m);
    assert_eq!(
        get_dispatch(&m, &key).unwrap().outcome,
        Some(Ok(b"result".to_vec()))
    );

    let mut sys = mk_ctx(6, Origin::System);
    exec(&mut m, &mut sys, &DispatchMsg::DeliverPending {}).unwrap();
    commit(&mut m);

    // the receiver got every byte...
    let event = crate::decode_result_event(&sys.msgs()[0].payload).unwrap();
    assert_eq!(event.outcome, Ok(b"result".to_vec()));
    // ...and the record kept none of them, while STAYING a record.
    let view = get_dispatch(&m, &key).expect("the delivered record survives");
    assert_eq!(view.status, DispatchStatus::Delivered);
    assert_eq!(view.outcome, None, "the delivered copy is dropped");
}

#[test]
fn sustained_dispatch_traffic_keeps_the_state_bounded_but_never_forgets_a_run() {
    // the growth pin. every dispatch record is `runs`' PERMANENT turn
    // claim (runs::dispatch_flow::turn_taken) — evicting one re-opens a
    // settled run for a duplicate agent launch — so the record count grows 1:1
    // with traffic ON PURPOSE, and per-record keys make that free: an op
    // touches only the keys it names. what must NOT grow is the RECORD: the
    // outcome is up to MAX_RESULT_BYTES and delivery drops this module's copy.
    let mut m = module();
    let mut ctx = mk_ctx(0, owner());
    exec(
        &mut m,
        &mut ctx,
        &register(OutputContract::Text, Routing::Rendezvous),
    )
    .unwrap();
    commit(&mut m);

    const ROUNDS: usize = 64;
    const OUTCOME_BYTES: usize = 64 * 1024;
    for i in 0..ROUNDS {
        full_round(&mut m, i, OUTCOME_BYTES);
    }
    assert_eq!(pending_deliveries(&m), 0, "every result was delivered");

    // ROUNDS * 64 KiB of results passed through; every receipt survives and
    // each is a FIXED-SIZE record, not a second ledger of the bytes.
    let mut total = 0;
    for i in 0..ROUNDS {
        let key = dispatch_key("caller", &format!("d{i:04}"));
        let view = get_dispatch(&m, &key).expect("no receipt is ever evicted");
        assert_eq!(view.status, DispatchStatus::Delivered);
        assert_eq!(view.outcome, None);
        total += record_bytes(&m, &key);
    }
    let ceiling = ROUNDS * 1024;
    assert!(
        total < ceiling,
        "the {ROUNDS} delivered records hold {total} bytes (ceiling {ceiling}); \
         delivered outcomes are being retained"
    );

    // the recipe is untouched.
    assert!(recipe(&m, "summarize").is_some());
}
