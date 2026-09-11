//! the two genesis parameters a fixed component cannot compile in, and what
//! each of them actually decides.
//!
//! `chain_id` binds every op to ONE network by name, so signed bytes cannot be
//! replayed onto another. `time_unit` decides what a delivery deadline is
//! measured in — the same seven days is 604_800 on a height lane and
//! 604_800_000 on a millisecond one, and a component that guessed would either
//! refuse every honest deadline or accept a decade-long one.

mod common;

use collaboration::{CollaborationMsg, max_delivery_ttl};
use common::*;
use futures::executor::block_on;
use sdk::{Module, Origin, genesis_config::TimeUnit};

#[test]
fn an_op_addressed_to_another_network_is_refused_before_it_is_read() {
    block_on(async {
        let Scene {
            mut module,
            chat,
            alice,
            ..
        } = scene("c1");
        let mut ctx = at(&chat, 1, Origin::External(key(1)));
        let refusal = module
            .execute(
                &mut ctx,
                &on_network("other-net", bind("c1", &alice, key(0x5e), 0)),
            )
            .await
            .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("not this network"),
            "{refusal:?}"
        );
        module.abort_block().await.unwrap();

        // the op is otherwise perfectly valid: name this network and it lands.
        ok(&mut module, &mut ctx, bind("c1", &alice, key(0x5e), 0)).await;
    });
}

#[test]
fn a_blank_chain_id_binds_no_network_and_admits_nothing() {
    // a component composed without a chain id knows no network, so it cannot
    // decide whether an op is meant for it. It must fail closed — matching the
    // blank would COLLAPSE every unnamed network into one replay domain, which
    // is exactly the thing the binding exists to prevent.
    block_on(async {
        let Scene { chat, alice, .. } = scene("c1");
        let mut module = module_on("", MAX_TTL);
        let mut ctx = at(&chat, 1, Origin::External(key(1)));
        for network in ["", NETWORK, "other-net"] {
            let refusal = module
                .execute(
                    &mut ctx,
                    &on_network(network, bind("c1", &alice, key(0x5e), 0)),
                )
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
        let scene = scene("c1");
        let bob = scene.bob.clone();
        scene.alice_posts("c1", "m1");
        let now = 1;
        let deadline = now + max_delivery_ttl(TimeUnit::Height) + 1;

        let mut height = module_on(NETWORK, max_delivery_ttl(TimeUnit::Height));
        let mut ctx = scene.as_alice(now);
        let refusal = apply(
            &mut height,
            &mut ctx,
            CollaborationMsg::Deliver(deliver("c1", "m1", &bob, deadline)),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("more than"),
            "the height lane must refuse a deadline past its ceiling: {refusal:?}"
        );

        let mut millis = module_on(NETWORK, max_delivery_ttl(TimeUnit::Millis));
        let mut ctx = scene.as_alice(now);
        ok(
            &mut millis,
            &mut ctx,
            CollaborationMsg::Deliver(deliver("c1", "m1", &bob, deadline)),
        )
        .await;

        // and the millisecond lane still HAS a ceiling — it is scaled, not
        // removed.
        scene.alice_posts("c1", "m2");
        let mut ctx = scene.as_alice(now);
        let refusal = apply(
            &mut millis,
            &mut ctx,
            CollaborationMsg::Deliver(deliver(
                "c1",
                "m2",
                &bob,
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
