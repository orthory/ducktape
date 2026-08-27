//! The netstack machine's frozen lifecycle suite. [`harness`] is the pure
//! world every scenario runs in; [`scenarios`] are the lifecycles
//! themselves, each pinned as a golden event→effect trace under
//! `fixtures/<scenario>.trace`. The trace IS the specification: a behavior
//! change is a PR that regenerates it (`UPDATE_TRACES=1 cargo test -p
//! netstack-scenarios`), and the review reads the fixture diff.
//!
//! The suite is generic over the [`Backend`] that builds each node's
//! machine, so ONE fixture set binds every backend: the native machine (this
//! crate's tests) and the wasm guest (`netstack-wasm`'s). A backend whose
//! trace differs by a byte fails exactly the way a behavior change does.

pub mod harness;
pub mod scenarios;

use netstack_machine::{Machine, MachineConfig, NetstackMachine};
use wireguard::IdentitySigner;

/// Builds one node's machine from its identity and config.
pub type Backend = fn(Box<dyn IdentitySigner>, MachineConfig) -> Box<dyn NetstackMachine>;

/// The native machine as a backend.
pub fn native(signer: Box<dyn IdentitySigner>, config: MachineConfig) -> Box<dyn NetstackMachine> {
    Box::new(Machine::new(signer, config))
}

/// One `#[test]` per scenario, each run on `$backend` against the shared
/// fixture. Invoked once per backend, in that backend's test crate; the
/// native lane's lint test keeps this list equal to [`scenarios`]'s.
#[macro_export]
macro_rules! suite {
    ($backend:expr) => {
        $crate::suite!(@each $backend;
            boot_two_members,
            boot_three_members,
            boot_five_members,
            boot_relay_through_hub,
            member_restart_mapping_kept,
            member_restart_mapping_lost,
            nat_rebind_readvertises,
            cutover_keeps_unchanged_peers,
            cutover_to_reduced_mesh,
            coordinator_dark,
            first_fanout_lost_both_sides,
            partition_across_cutover_heals_on_reconnect,
            handshake_lost_at_each_stage,
            duplicated_delivery_is_tolerated,
            standby_prewarm_then_promotion,
            slow_resolver_does_not_stall,
            join_direct_invite,
            join_coordinated_invite,
        );
    };
    (@each $backend:expr; $($name:ident),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                $crate::scenarios::$name($backend);
            }
        )*
    };
}
