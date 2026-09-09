//! the two genesis parameters a fixed component cannot compile in, and what
//! each of them actually decides.
//!
//! `chain_id` binds every op to ONE network by name, so signed bytes cannot be
//! replayed onto another. `time_unit` decides what a delivery deadline is
//! measured in — the same seven days is 604_800 on a height lane and
//! 604_800_000 on a millisecond one, and a component that guessed would either
//! refuse every honest deadline or accept a decade-long one.

mod common;

use collaboration::{max_delivery_ttl, CollaborationMsg};
use common::*;
use futures::executor::block_on;
use sdk::{genesis_config::TimeUnit, Module, Origin};

#[test]
fn an_op_addressed_to_another_network_is_refused_before_it_is_read() {
    block_on(async {
        let mut module = module();
        let mut ctx = at(1, Origin::External(key(1)));
        let refusal = module
            .execute(&mut ctx, &on_network("other-net", register("alice")))
            .await
            .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("not this network"),
            "{refusal:?}"
        );

        // the op is otherwise perfectly valid: name this network and it lands.
        ok(&mut module, &mut ctx, register("alice")).await;
    });
}

#[test]
fn a_blank_chain_id_binds_no_network_and_admits_nothing() {
    // a component composed without a chain id knows no network, so it cannot
    // decide whether an op is meant for it. It must fail closed — matching the
    // blank would COLLAPSE every unnamed network into one replay domain, which
    // is exactly the thing the binding exists to prevent.
    block_on(async {
        let mut module = module_on("", MAX_TTL);
        let mut ctx = at(1, Origin::External(key(1)));
        for network in ["", NETWORK, "other-net"] {
            let refusal = module
                .execute(&mut ctx, &on_network(network, register("alice")))
                .await
                .unwrap_err();
            assert!(
                format!("{refusal:?}").contains("binds no network"),
                "an op naming {network:?} must not be admitted by a blank binding: {refusal:?}"
            );
            module.abort_block().await.unwrap();
        }
    });
}

#[test]
fn the_time_unit_scales_the_delivery_ceiling_it_does_not_relabel_it() {
    // the same wall-clock span, in each lane's own unit. a height lane counts
    // blocks and a millisecond lane counts milliseconds, so the ceiling that
    // means "seven days" differs by exactly the unit's rate.
    assert_eq!(TimeUnit::Height.per_second(), 1);
    assert_eq!(TimeUnit::Millis.per_second(), 1_000);
    assert_eq!(
        max_delivery_ttl(TimeUnit::Millis),
        max_delivery_ttl(TimeUnit::Height) * 1_000
    );
}

#[test]
fn a_millis_lane_admits_the_deadline_a_height_lane_calls_too_far_out() {
    // THE LANE TEST. One deadline, two networks. Under a height ceiling it is
    // absurdly far out; under a millisecond ceiling it is a few minutes. A
    // single compiled-in constant would have to be wrong on one of them — which
    // is why the ceiling rides genesis config instead.
    block_on(async {
        let now = 1;
        let deadline = now + max_delivery_ttl(TimeUnit::Height) + 1;

        let mut height = seated(module_on(NETWORK, max_delivery_ttl(TimeUnit::Height))).await;
        let mut ctx = at(now, Origin::External(key(1)));
        let refusal = apply(
            &mut height,
            &mut ctx,
            CollaborationMsg::Send(note("c1", "alice", "bob", 1, 1, deadline)),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("more than"),
            "the height lane must refuse a deadline past its ceiling: {refusal:?}"
        );

        let mut millis = seated(module_on(NETWORK, max_delivery_ttl(TimeUnit::Millis))).await;
        let mut ctx = at(now, Origin::External(key(1)));
        ok(
            &mut millis,
            &mut ctx,
            CollaborationMsg::Send(note("c1", "alice", "bob", 1, 1, deadline)),
        )
        .await;

        // and the millisecond lane still HAS a ceiling — it is scaled, not
        // removed.
        let mut ctx = at(now, Origin::External(key(1)));
        let refusal = apply(
            &mut millis,
            &mut ctx,
            CollaborationMsg::Send(note(
                "c1",
                "alice",
                "bob",
                1,
                2,
                now + max_delivery_ttl(TimeUnit::Millis) + 1,
            )),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("more than"),
            "{refusal:?}"
        );
    });
}

/// the `scene` setup against a caller-supplied module: two participants owned
/// by two keys, both seated on `c1`.
async fn seated(mut module: collaboration::Collaboration) -> collaboration::Collaboration {
    let mut owner_a = at(1, Origin::External(key(1)));
    ok(&mut module, &mut owner_a, register("alice")).await;
    ok(&mut module, &mut owner_a, conversation("c1")).await;
    let mut owner_b = at(1, Origin::External(key(2)));
    ok(&mut module, &mut owner_b, register("bob")).await;
    for participant in ["alice", "bob"] {
        ok(
            &mut module,
            &mut owner_a,
            seat("c1", participant, Some(collaboration::Role::Member)),
        )
        .await;
    }
    module
}
