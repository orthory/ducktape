//! the lifecycle module's public wire surface — types only.
//!
//! lifecycle is the MODULE CODE coordination plane, folded into ONE root-hashed
//! `root()`: per hot-swappable module, the ACTIVE 32-byte code hash plus at
//! most one pending `ScheduledSwap`; governance authorizes a
//! register/schedule/cancel, each validator emits `SwapReady` once the target
//! bytes are verified-resident, and the boundary `Advance` tick activates every
//! swap whose `activation_height` has been reached AND whose readiness latched
//! R=n. the code BYTES are out-of-band (content-addressed by the 32-byte
//! hash); this module is the consensus commitment to WHICH code is active,
//! never the bytes.

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

/// the genesis-constant module id the lifecycle module registers under. the host
/// reads it to reconcile running code against the committed active hashes and
/// inject the boundary `Advance`; governance addresses its authorized
/// follow-ups here.
pub const DEFAULT_LIFECYCLE_ID: &str = "lifecycle";

/// the length of a code hash: sha256 over the component bytes.
pub const CODE_HASH_LEN: usize = 32;

// ---- the module-code path shapes --------------------------------------------

/// coordinates of a scheduled code swap for one module. **at most one** is ever
/// pending per module.
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
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

/// one activation: `code_hash` became the module's running code FOR block
/// `height` — the block whose boundary `Advance` flipped it (a `RegisterModule`
/// records its own block; a genesis seed records 0). appended, never
/// rewritten: the registry is disk-durable and reopens AHEAD of a crash-restart
/// replay, so only a history can answer "which code sealed block h" once the
/// swap that replaced it has landed. committed state, in the root.
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Activation {
    pub height: u64,
    /// the 32-byte sha256 of the component bytes.
    pub code_hash: Vec<u8>,
}

/// the readable per-module projection.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModuleCode {
    pub module_id: String,
    pub active_code_hash: Vec<u8>,
    pub pending: Option<ScheduledSwap>,
    /// every activation in block order; `active_code_hash` is its last entry
    /// (empty only for an admission that has not reached its boundary).
    pub history: Vec<Activation>,
}

/// the code designated for block `height` — what a node must RUN to apply it.
/// a pending swap armed at `height` (ready, activation reached) wins: that is
/// the live pre-flip read, the boundary of the very block that flips it, and
/// the flip then records the same `(height, code_hash)`. otherwise the latest
/// activation at or before `height` — the replay read against a registry that
/// is already AHEAD. a module whose first activation is later than `height`
/// seats its first code (it has no ops before it). `None` is a module
/// registered but never activated.
pub fn code_at(entry: &ModuleCode, height: u64) -> Option<&[u8]> {
    let armed = entry
        .pending
        .as_ref()
        .filter(|p| p.ready && height >= p.activation_height);
    if let Some(p) = armed {
        return Some(&p.code_hash);
    }
    let sealed_at_or_before = entry.history.iter().rev().find(|a| a.height <= height);
    let seat = sealed_at_or_before.or_else(|| entry.history.first());
    seat.map(|a| a.code_hash.as_slice())
}

/// one armed swap the host must realize: swap `module_id`'s registry code to
/// `code_hash` at the boundary.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArmedSwap {
    pub module_id: String,
    pub code_hash: Vec<u8>,
}

// ---- the wire surface -------------------------------------------------------

/// what an ingested op DOES to the lifecycle module. the ORIGIN is the
/// authority, not the variant: schedule/cancel/register are
/// governance/system-authored, `SwapReady` is validator-authored, and `Advance`
/// is the system-injected boundary tick.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum LifecycleMsg {
    /// install a module's INITIAL active code hash (genesis/bootstrap). rejects a
    /// re-register of a known module — code changes go through `ScheduleSwap`.
    /// `Origin::Module("governance") | System` only.
    RegisterModule {
        module_id: String,
        code_hash: Vec<u8>,
    },
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

    /// the system-injected boundary tick, keyed on `env.height`: activate every
    /// armed code swap. `Origin::System` only.
    Advance,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum LifecycleQuery {
    /// active + pending code for every registered module.
    ModuleStatus,
    /// the swaps ARMED at `height` (`activation_height <= height`). the host reads
    /// this at the boundary to know which registry modules to swap and to which
    /// code hash.
    ArmedAt { height: u64 },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum LifecycleReply {
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
    use super::*;

    fn rt_msg(m: LifecycleMsg) {
        assert_eq!(decode_msg(&encode_msg(&m)).unwrap(), m);
    }

    #[test]
    fn msg_query_reply_round_trip_every_variant() {
        rt_msg(LifecycleMsg::RegisterModule {
            module_id: "hello".into(),
            code_hash: vec![1u8; CODE_HASH_LEN],
        });
        rt_msg(LifecycleMsg::ScheduleSwap {
            name: "swap-hello".into(),
            module_id: "hello".into(),
            activation_height: 10,
            code_hash: vec![2u8; CODE_HASH_LEN],
        });
        rt_msg(LifecycleMsg::ScheduleRegister {
            name: "admit-kanban".into(),
            module_id: "kanban".into(),
            activation_height: 10,
            code_hash: vec![5u8; CODE_HASH_LEN],
        });
        rt_msg(LifecycleMsg::CancelSwap {
            name: "swap-hello".into(),
            module_id: "hello".into(),
        });
        rt_msg(LifecycleMsg::SwapReady {
            name: "swap-hello".into(),
            module_id: "hello".into(),
        });
        rt_msg(LifecycleMsg::Advance);

        for q in [
            LifecycleQuery::ModuleStatus,
            LifecycleQuery::ArmedAt { height: 9 },
        ] {
            assert_eq!(decode_query(&encode_query(&q)).unwrap(), q);
        }

        let r = LifecycleReply::ModuleStatus {
            modules: vec![ModuleCode {
                module_id: "hello".into(),
                active_code_hash: vec![1u8; CODE_HASH_LEN],
                pending: None,
                history: vec![Activation {
                    height: 0,
                    code_hash: vec![1u8; CODE_HASH_LEN],
                }],
            }],
        };
        assert_eq!(decode_reply(&encode_reply(&r)).unwrap(), r);
    }
}
