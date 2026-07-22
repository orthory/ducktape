//! the lifecycle module's public wire surface — types only.
//!
//! lifecycle merges the two mirror-image coordination classes that used to live
//! in the `upgrade` and `modreg` crates into ONE wire surface, folded into ONE
//! app-hashed `root()`:
//!
//!   * the node PROTOCOL VERSION path — the agreed `current_version`, the single
//!     pending `ScheduledUpgrade { name, activation_height, to_version }`, and
//!     the per-validator readiness set; governance authorizes a
//!     schedule/cancel, each validator emits `UpgradeReady` once running the new
//!     binary, and the boundary `Advance` tick arms (flips `current_version`)
//!     iff every boundary member signaled, else aborts.
//!
//!   * the MODULE CODE path — per hot-swappable module, the ACTIVE 32-byte code
//!     hash plus at most one pending `ScheduledSwap`; governance authorizes a
//!     register/schedule/cancel, each validator emits `SwapReady` once the
//!     target bytes are verified-resident, and the same boundary `Advance`
//!     activates every swap whose `activation_height` has been reached AND whose
//!     readiness latched R=n. the code BYTES are out-of-band (content-addressed
//!     by the 32-byte hash); this module is the consensus commitment to WHICH
//!     code is active, never the bytes.
//!
//! the two paths share exactly one input variant — the system-injected `Advance`
//! boundary tick, which reconciles BOTH halves in one dispatch.

use serde::{Deserialize, Serialize};

/// the genesis-constant module id the lifecycle module registers under. the host
/// reads it to derive the block protocol version, reconcile running code against
/// the committed active hashes, and inject the boundary `Advance`; governance
/// addresses its authorized follow-ups here.
pub const DEFAULT_LIFECYCLE_ID: &str = "lifecycle";

/// the length of a code hash: sha256 over the component bytes.
pub const CODE_HASH_LEN: usize = 32;

/// the protocol version post-genesis module ADMISSION activates at. below it,
/// `ScheduleRegister` (and governance's `RegisterModule` door) refuse
/// deterministically — which closes the mixed-binary window: an old binary
/// rejects the op because it cannot decode it, a new binary rejects it here,
/// and both leave state untouched. deliberately ABOVE every shipping
/// `MAX_PROTOCOL_VERSION` (a compile-time assert in `bin/node/src/constants.rs`
/// enforces it): raising the network past this version is the deliberate act
/// that turns admissions on, and it must not happen before the recovery /
/// state-sync composition can restore an admitted module's accumulated state
/// (today's composers enumerate a fixed module set — the restore half of
/// admissions is follow-up work).
pub const ADMISSION_ACTIVATION_VERSION: u32 = 4;

// ---- the protocol-version path shapes ---------------------------------------

/// the coordinates of a scheduled node upgrade. **at most one** is ever pending.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ScheduledUpgrade {
    pub name: String,
    pub activation_height: u64,
    pub to_version: u32,
}

/// Upgrade names beginning with this prefix bind readiness to the remaining
/// bytes. Binaries that do not implement the named route keep emitting
/// `commitment: None`, so they cannot arm a committed-route upgrade merely by
/// advertising the same numeric protocol ceiling.
pub const READINESS_COMMITMENT_PREFIX: &str = "commit:";

pub fn required_readiness_commitment(name: &str) -> Option<&[u8]> {
    name.strip_prefix(READINESS_COMMITMENT_PREFIX)
        .map(str::as_bytes)
}

pub fn readiness_commitment_matches(name: &str, commitment: Option<&[u8]>) -> bool {
    required_readiness_commitment(name).is_none_or(|expected| commitment == Some(expected))
}

/// the readable projection of the upgrade-path state.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct UpgradeStatus {
    pub current_version: u32,
    pub pending: Option<ScheduledUpgrade>,
    /// the boundary member set the verdict was computed against, sorted. lets a
    /// caller (e.g. the host stamping seam) re-run the shared `effective_version`.
    pub members: Vec<Vec<u8>>,
    /// commitment-valid readiness keys for the pending upgrade, sorted.
    pub ready: Vec<Vec<u8>>,
    pub member_count: u64,
    pub ready_count: u64,
    /// `pending.is_some() && members non-empty && every boundary member has a
    /// commitment-valid entry in `ready`, derived from the shared
    /// `effective_version` predicate (no hand-copied logic).
    pub armed: bool,
}

/// the ONE arming predicate, as a pure/total function of committed coordinates:
/// no IO, clock, or RNG; identical on every node (live, replay, state-sync). the
/// module's `Advance` handler routes through this, so the version derivation can
/// never drift from the arm check (risk R4). returns `pending.to_version` when
/// the pending upgrade is ARMED at `height` — `pending.is_some()`,
/// `height >= activation_height`, `boundary_members` non-empty, and every
/// boundary member present in `ready` — otherwise `current`.
pub fn effective_version(
    height: u64,
    current: u32,
    pending: Option<&ScheduledUpgrade>,
    boundary_members: &[Vec<u8>],
    is_ready: impl Fn(&[u8]) -> bool,
) -> u32 {
    match pending {
        Some(up)
            if height >= up.activation_height
                && !boundary_members.is_empty()
                && boundary_members.iter().all(|member| is_ready(member)) =>
        {
            up.to_version
        }
        _ => current,
    }
}

// ---- the module-code path shapes --------------------------------------------

/// coordinates of a scheduled code swap for one module. **at most one** is ever
/// pending per module.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ScheduledSwap {
    pub name: String,
    pub activation_height: u64,
    /// the 32-byte sha256 of the target component bytes.
    pub code_hash: Vec<u8>,
    /// validator pubkeys that verified the target BYTES locally and signaled
    /// (`LifecycleMsg::SwapReady`), strictly increasing. committed state, in
    /// the root like everything else here.
    pub readiness: Vec<Vec<u8>>,
    /// LATCHED true the moment `readiness` covers the whole boundary member
    /// set (R = n, evaluated at signal time). the arm predicate is
    /// `ready && activation_height <= height` — a swap never activates onto
    /// a validator set that has not demonstrably received the bytes. a
    /// member admitted AFTER the latch heals through the fetch lane
    /// (fail-closed backstop) rather than blocking the swap.
    pub ready: bool,
}

/// the readable per-module projection.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ModuleCode {
    pub module_id: String,
    pub active_code_hash: Vec<u8>,
    pub pending: Option<ScheduledSwap>,
}

/// one armed swap the host must realize: swap `module_id`'s registry code to
/// `code_hash` at the boundary.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ArmedSwap {
    pub module_id: String,
    pub code_hash: Vec<u8>,
}

// ---- the unified wire surface -----------------------------------------------

/// what an ingested op DOES to the lifecycle module. the ORIGIN is the
/// authority, not the variant: schedule/cancel/register are
/// governance/system-authored, the `*Ready` signals are validator-authored, and
/// `Advance` is the system-injected boundary tick that reconciles BOTH paths.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleMsg {
    // ---- protocol-version path ----
    /// authorize a pending node upgrade. `Origin::Module("governance") | System`.
    ScheduleUpgrade {
        name: String,
        activation_height: u64,
        to_version: u32,
    },
    /// clear a pending upgrade before its boundary. `Origin::Module | System`.
    CancelUpgrade { name: String },
    /// a validator records readiness for the pending upgrade.
    /// `Origin::External(pubkey)` of a CURRENT boundary member only.
    UpgradeReady {
        name: String,
        to_version: u32,
        commitment: Option<Vec<u8>>,
    },

    // ---- module-code path ----
    /// install a module's INITIAL active code hash (genesis/bootstrap). rejects a
    /// re-register of a known module — code changes go through `ScheduleSwap`.
    /// `Origin::Module("governance") | System` only.
    RegisterModule { module_id: String, code_hash: Vec<u8> },
    /// schedule a height-gated code swap for a registered module.
    /// `Origin::Module | System` only.
    ScheduleSwap {
        name: String,
        module_id: String,
        activation_height: u64,
        code_hash: Vec<u8>,
    },
    /// schedule the ADMISSION of a brand-new module post-genesis: creates the
    /// entry with an EMPTY active hash and this pending, readiness-latched
    /// initial code. the module has no running code until the boundary realizes
    /// the swap; the host instantiates it from the fetched bytes at activation.
    /// cancelling before the boundary removes the entry entirely.
    /// `Origin::Module | System` only.
    ScheduleRegister {
        name: String,
        module_id: String,
        activation_height: u64,
        code_hash: Vec<u8>,
    },
    /// clear a pending swap before its boundary. `Origin::Module | System` only.
    CancelSwap { name: String, module_id: String },
    /// a validator records that it HOLDS (verified locally) the pending swap's
    /// component bytes. `Origin::External(validator)` only, member-gated against
    /// the valset; the last covering signal latches the swap `ready`.
    SwapReady { name: String, module_id: String },

    // ---- shared boundary tick ----
    /// the system-injected boundary tick, keyed on `env.height`: arm/abort the
    /// pending upgrade AND activate every armed code swap. `Origin::System` only.
    Advance,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleQuery {
    /// the current version, the pending upgrade, and the readiness verdict.
    UpgradeStatus,
    /// active + pending code for every registered module.
    ModuleStatus,
    /// the swaps ARMED at `height` (`activation_height <= height`). the host reads
    /// this at the boundary to know which registry modules to swap and to which
    /// code hash.
    ArmedAt { height: u64 },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleReply {
    UpgradeStatus(UpgradeStatus),
    ModuleStatus { modules: Vec<ModuleCode> },
    ArmedAt { swaps: Vec<ArmedSwap> },
}

pub fn encode_msg(m: &LifecycleMsg) -> Vec<u8> {
    sdk::wire::encode(m)
}
pub fn decode_msg(b: &[u8]) -> Result<LifecycleMsg, String> {
    sdk::wire::decode(b)
}
pub fn encode_query(q: &LifecycleQuery) -> Vec<u8> {
    sdk::wire::encode(q)
}
pub fn decode_query(b: &[u8]) -> Result<LifecycleQuery, String> {
    sdk::wire::decode(b)
}
pub fn encode_reply(r: &LifecycleReply) -> Vec<u8> {
    sdk::wire::encode(r)
}
pub fn decode_reply(b: &[u8]) -> Result<LifecycleReply, String> {
    sdk::wire::decode(b)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn rt_msg(m: LifecycleMsg) {
        assert_eq!(decode_msg(&encode_msg(&m)).unwrap(), m);
    }

    fn has(keys: &BTreeMap<Vec<u8>, ()>) -> impl Fn(&[u8]) -> bool + '_ {
        |member| keys.contains_key(member)
    }

    #[test]
    fn msg_query_reply_round_trip_every_variant() {
        rt_msg(LifecycleMsg::ScheduleUpgrade {
            name: "forge-multi-repo".into(),
            activation_height: 100,
            to_version: 2,
        });
        rt_msg(LifecycleMsg::CancelUpgrade {
            name: "forge-multi-repo".into(),
        });
        rt_msg(LifecycleMsg::UpgradeReady {
            name: "forge-multi-repo".into(),
            to_version: 2,
            commitment: None,
        });
        rt_msg(LifecycleMsg::UpgradeReady {
            name: "x".into(),
            to_version: 3,
            commitment: Some(vec![1, 2, 3]),
        });
        rt_msg(LifecycleMsg::RegisterModule {
            module_id: "hello".into(),
            code_hash: vec![1u8; CODE_HASH_LEN],
        });
        rt_msg(LifecycleMsg::ScheduleSwap {
            name: "v2".into(),
            module_id: "hello".into(),
            activation_height: 10,
            code_hash: vec![2u8; CODE_HASH_LEN],
        });
        rt_msg(LifecycleMsg::ScheduleRegister {
            name: "v1".into(),
            module_id: "kanban".into(),
            activation_height: 10,
            code_hash: vec![5u8; CODE_HASH_LEN],
        });
        rt_msg(LifecycleMsg::CancelSwap {
            name: "v2".into(),
            module_id: "hello".into(),
        });
        rt_msg(LifecycleMsg::SwapReady {
            name: "v2".into(),
            module_id: "hello".into(),
        });
        rt_msg(LifecycleMsg::Advance);

        for q in [
            LifecycleQuery::UpgradeStatus,
            LifecycleQuery::ModuleStatus,
            LifecycleQuery::ArmedAt { height: 9 },
        ] {
            assert_eq!(decode_query(&encode_query(&q)).unwrap(), q);
        }

        let r = LifecycleReply::UpgradeStatus(UpgradeStatus {
            current_version: 1,
            pending: Some(ScheduledUpgrade {
                name: "n".into(),
                activation_height: 9,
                to_version: 2,
            }),
            members: vec![vec![1u8; 32], vec![2u8; 32]],
            ready: vec![vec![1u8; 32], vec![2u8; 32]],
            member_count: 2,
            ready_count: 2,
            armed: true,
        });
        assert_eq!(decode_reply(&encode_reply(&r)).unwrap(), r);
    }

    #[test]
    fn effective_version_truth_table() {
        let m1 = vec![1u8; 32];
        let m2 = vec![2u8; 32];
        let members = vec![m1.clone(), m2.clone()];
        let up = ScheduledUpgrade {
            name: "n".into(),
            activation_height: 10,
            to_version: 2,
        };

        // no pending -> current.
        let empty: BTreeMap<Vec<u8>, ()> = BTreeMap::new();
        assert_eq!(effective_version(10, 1, None, &members, has(&empty)), 1);

        // armed & height < activation -> current.
        let mut all: BTreeMap<Vec<u8>, ()> = BTreeMap::new();
        all.insert(m1.clone(), ());
        all.insert(m2.clone(), ());
        assert_eq!(effective_version(9, 1, Some(&up), &members, has(&all)), 1);

        // armed & height >= activation & all members ready -> to_version.
        assert_eq!(effective_version(10, 1, Some(&up), &members, has(&all)), 2);

        // a boundary member missing -> current.
        let mut partial: BTreeMap<Vec<u8>, ()> = BTreeMap::new();
        partial.insert(m1.clone(), ());
        assert_eq!(
            effective_version(10, 1, Some(&up), &members, has(&partial)),
            1
        );

        // empty boundary set -> current (never arm against no members).
        assert_eq!(effective_version(10, 1, Some(&up), &[], has(&all)), 1);
    }
}
