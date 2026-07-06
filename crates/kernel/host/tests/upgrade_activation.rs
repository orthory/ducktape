//! Phase 6, Task 6.3 — the deterministic, in-block System-origin `Advance`
//! injection at a finalized activation boundary.
//!
//! The host's drain injects EXACTLY ONE `Origin::System` `UpgradeMsg::Advance`
//! whenever the committed `upgrade` module holds a pending upgrade that has
//! reached its `activation_height`. Because it rides the SAME `submit_at` drain
//! that recovery-replay and state-sync-install also run, the reconciliation
//! (ARM: flip `current_version` + clear pending/readiness; ABORT: clear only)
//! reconstructs byte-for-byte on every node. This frees the at-most-one-pending
//! slot after a successful activation.

use commonware_cryptography::Signer as _;
use commonware_cryptography::ed25519::PrivateKey;
use futures::executor::block_on;

use host::{BASELINE_VERSION, BlockContext, Host, SubmitError};
use sdk::{Msg, Origin};
use upgrade::Upgrade;
use upgrade::{
    UpgradeMsg, UpgradeQuery, UpgradeReply, UpgradeStatus, encode_msg, encode_query,
};
use valset::Valset;

fn key(seed: u64) -> Vec<u8> {
    PrivateKey::from_seed(seed).public_key().as_ref().to_vec()
}

/// a host with a genesis-seeded valset (`members`) + a fresh upgrade module.
fn host_with(members: &[Vec<u8>]) -> Host {
    let mut valset = Valset::new("valset");
    for m in members {
        valset.insert(m.clone());
    }
    let mut host = Host::new();
    host.register(Box::new(valset));
    host.register(Box::new(Upgrade::new("upgrade", "valset")));
    host
}

fn submit(host: &mut Host, height: u64, origin: Origin, msg: Msg) {
    let ctx = BlockContext {
        height,
        consensus_time: height,
        origin,
        protocol_version: BASELINE_VERSION,
    };
    block_on(host.submit_at(ctx, msg)).expect("block applies");
}

fn schedule_msg(name: &str, activation_height: u64, to_version: u32) -> Msg {
    Msg {
        target: "upgrade".into(),
        payload: encode_msg(&UpgradeMsg::Schedule {
            name: name.into(),
            activation_height,
            to_version,
        }),
    }
}

fn signal_msg(name: &str, to_version: u32) -> Msg {
    Msg {
        target: "upgrade".into(),
        payload: encode_msg(&UpgradeMsg::SignalReady {
            name: name.into(),
            to_version,
            commitment: None,
        }),
    }
}

fn status(host: &Host) -> UpgradeStatus {
    let reply = block_on(host.query("upgrade", &encode_query(&UpgradeQuery::Status)))
        .expect("status query");
    match upgrade::decode_reply(&reply).expect("decode status") {
        UpgradeReply::Status(s) => s,
    }
}

/// ARM: at a boundary where every member signaled, the injected `Advance` flips
/// `current_version`, clears the pending slot, and frees it for a second schedule.
#[test]
fn advance_injection_arms_and_frees_slot() {
    let m0 = key(1);
    let m1 = key(2);
    let members = vec![m0.clone(), m1.clone()];
    let mut host = host_with(&members);

    // schedule (governance/system authored) + every member signals ready.
    submit(&mut host, 0, Origin::System, schedule_msg("forge-multi-repo", 10, 2));
    submit(&mut host, 1, Origin::External(m0.clone()), signal_msg("forge-multi-repo", 2));
    submit(&mut host, 2, Origin::External(m1.clone()), signal_msg("forge-multi-repo", 2));
    let before = status(&host);
    assert_eq!(before.current_version, 0);
    assert!(before.pending.is_some());
    assert!(before.armed, "R == n before the boundary");

    // a block AT the activation height: the root op is a (harmless, idempotent)
    // re-signal; the drain then injects the System Advance in the SAME block.
    submit(&mut host, 10, Origin::External(m0.clone()), signal_msg("forge-multi-repo", 2));
    let after = status(&host);
    assert_eq!(after.current_version, 2, "Advance flipped to to_version at H");
    assert!(after.pending.is_none(), "pending cleared");
    assert_eq!(after.ready_count, 0, "readiness cleared");

    // the slot is free: a fresh, higher schedule is now accepted.
    submit(&mut host, 11, Origin::System, schedule_msg("next", 30, 3));
    assert!(status(&host).pending.is_some(), "second schedule accepted");
}

/// ABORT: at a boundary with an unmet quorum, the injected `Advance` clears the
/// pending slot WITHOUT flipping `current_version`.
#[test]
fn advance_injection_aborts_on_unmet_quorum() {
    let m0 = key(1);
    let m1 = key(2);
    let members = vec![m0.clone(), m1.clone()];
    let mut host = host_with(&members);

    submit(&mut host, 0, Origin::System, schedule_msg("forge-multi-repo", 10, 2));
    // only m0 signals — m1 is a straggler.
    submit(&mut host, 1, Origin::External(m0.clone()), signal_msg("forge-multi-repo", 2));
    assert!(!status(&host).armed, "R < n");

    // at H the drain injects Advance; the root op is a neutral re-signal.
    submit(&mut host, 10, Origin::External(m0.clone()), signal_msg("forge-multi-repo", 2));
    let after = status(&host);
    assert_eq!(after.current_version, 0, "abort leaves current_version unchanged");
    assert!(after.pending.is_none(), "abort clears pending");
    assert_eq!(after.ready_count, 0, "abort clears readiness");

    // the slot is free after a clean abort too.
    submit(&mut host, 11, Origin::System, schedule_msg("retry", 30, 2));
    assert!(status(&host).pending.is_some());
}

/// below `H` the drain injects NOTHING — the version does not flip early, even
/// with a full readiness set.
#[test]
fn no_injection_below_activation_height() {
    let m0 = key(1);
    let members = vec![m0.clone()];
    let mut host = host_with(&members);

    submit(&mut host, 0, Origin::System, schedule_msg("n", 10, 2));
    submit(&mut host, 1, Origin::External(m0.clone()), signal_msg("n", 2));
    let root_before = host.module_root("upgrade");

    // a block at height 9 (< 10): no Advance, no flip, root unmoved.
    submit(&mut host, 9, Origin::External(m0.clone()), signal_msg("n", 2));
    let after = status(&host);
    assert_eq!(after.current_version, 0);
    assert!(after.pending.is_some());
    assert_eq!(host.module_root("upgrade"), root_before, "root unmoved below H");
}

/// INERT before the retrofit: a host WITHOUT the upgrade module registered runs
/// the drain byte-identically — the injection query errors and yields no op, so a
/// block at any height applies (or cleanly rejects) with no upgrade side-effect.
#[test]
fn inert_without_upgrade_module() {
    // only valset registered (no upgrade module).
    let m0 = key(1);
    let mut valset = Valset::new("valset");
    valset.insert(m0.clone());
    let mut host = Host::new();
    host.register(Box::new(valset));

    let hash_before = host.app_hash();
    // any block at any height: the missing upgrade module means no injection. an
    // op to an unregistered target is a deterministic REJECTION (never a panic /
    // Fatal), and a rejected block leaves no trace — the app-hash is unmoved.
    let ctx = BlockContext {
        height: 10,
        consensus_time: 10,
        origin: Origin::External(m0),
        protocol_version: BASELINE_VERSION,
    };
    let err = block_on(host.submit_at(ctx, Msg { target: "nop".into(), payload: Vec::new() }))
        .expect_err("unknown target rejects");
    assert!(matches!(err, SubmitError::Rejected(_)), "clean rejection, not Fatal");
    assert_eq!(host.app_hash(), hash_before, "no upgrade activity, hash unmoved");
}

/// If the finalized ROOT op at exactly `H` is REJECTED (so the whole block aborts
/// under host-lent atomicity), the injected `Advance` is rolled back with it, so
/// activation does NOT fire at `H`. It re-injects and lands at the first APPLIED
/// block `>= H`. This is a deterministic, self-healing "activation binds to the
/// first applied block at/after H" semantics: every node sees the identical
/// finalized op stream, so the identical accept/reject and therefore the identical
/// activation height — no fork, and the pending slot still frees on the boundary
/// crossing (an adversary who lands a rejected op at H defers activation by one
/// block, never permanently blocks it).
#[test]
fn activation_defers_when_the_carrier_block_at_h_aborts() {
    let m0 = key(1);
    let m1 = key(2);
    let members = vec![m0.clone(), m1.clone()];
    let mut host = host_with(&members);

    submit(&mut host, 0, Origin::System, schedule_msg("forge-multi-repo", 10, 2));
    submit(&mut host, 1, Origin::External(m0.clone()), signal_msg("forge-multi-repo", 2));
    submit(&mut host, 2, Origin::External(m1.clone()), signal_msg("forge-multi-repo", 2));
    assert!(status(&host).armed, "R == n before the boundary");

    // a block AT H=10 whose ROOT op is REJECTED (a SignalReady from a non-member):
    // execute errors -> the whole block aborts -> the injected Advance rolls back
    // too, so nothing activates at H.
    let nonmember = key(99);
    let ctx = BlockContext {
        height: 10,
        consensus_time: 10,
        origin: Origin::External(nonmember),
        protocol_version: BASELINE_VERSION,
    };
    let err = block_on(host.submit_at(ctx, signal_msg("forge-multi-repo", 2)))
        .expect_err("a non-member SignalReady rejects the whole block at H");
    assert!(matches!(err, SubmitError::Rejected(_)));
    let deferred = status(&host);
    assert_eq!(deferred.current_version, 0, "an aborted carrier block defers activation");
    assert!(deferred.pending.is_some(), "pending survives the aborted block");

    // the NEXT applied block (>= H) re-injects the Advance and activation lands —
    // deterministic self-heal; the slot then frees.
    submit(&mut host, 11, Origin::External(m0.clone()), signal_msg("forge-multi-repo", 2));
    let healed = status(&host);
    assert_eq!(
        healed.current_version, 2,
        "activation self-heals at the first APPLIED block >= H"
    );
    assert!(healed.pending.is_none(), "pending cleared on the healed boundary");
}
