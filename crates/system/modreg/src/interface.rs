//! the modreg module's public wire surface — types only.
//!
//! modreg holds, per hot-swappable module, the ACTIVE code hash and at most one
//! pending, height-gated code swap — all folded into the app-hash. governance
//! authorizes a schedule/cancel by emitting a host-drained follow-up (origin
//! `Module("governance")`); the system-injected `Advance` boundary tick
//! activates every swap whose `activation_height` has been reached. the code
//! BYTES are out-of-band (content-addressed by the 32-byte hash); this module is
//! the consensus commitment to WHICH code is active, never the bytes.

use serde::{Deserialize, Serialize};

/// the genesis-constant module id the code registry registers under. the host
/// reads it (via `Host::realize_module_swaps`) to reconcile running code against
/// the committed active hashes; governance addresses its `Schedule` follow-ups
/// here.
pub const DEFAULT_MODREG_ID: &str = "modreg";

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
/// admissions is follow-up work). this ceiling FLOATS above whatever feature
/// claims the next slot: continuation transactions took v4
/// (`node::CONTINUATION_ACTIVATION_VERSION`), which pushed admission to 5 —
/// pinning admission at 4 would have made the continuation flag day cross the
/// still-blocked admission boundary as a side effect.
pub const ADMISSION_ACTIVATION_VERSION: u32 = 5;

/// coordinates of a scheduled code swap for one module. **at most one** is ever
/// pending per module.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ScheduledSwap {
    pub name: String,
    pub activation_height: u64,
    /// the 32-byte sha256 of the target component bytes.
    pub code_hash: Vec<u8>,
    /// validator pubkeys that verified the target BYTES locally and signaled
    /// (`ModregMsg::SignalReady`), strictly increasing. committed state, in
    /// the root like everything else here.
    #[serde(default)]
    pub readiness: Vec<Vec<u8>>,
    /// LATCHED true the moment `readiness` covers the whole boundary member
    /// set (R = n, evaluated at signal time). the arm predicate is
    /// `ready && activation_height <= height` — a swap never activates onto
    /// a validator set that has not demonstrably received the bytes. a
    /// member admitted AFTER the latch heals through the fetch lane
    /// (fail-closed backstop) rather than blocking the swap.
    #[serde(default)]
    pub ready: bool,
}

/// what an ingested op DOES to modreg. the ORIGIN is the authority, not the
/// variant: register/schedule/cancel are governance/system-authored, `Advance`
/// is the system-injected boundary tick.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModregMsg {
    /// install a module's INITIAL active code hash (genesis/bootstrap). rejects a
    /// re-register of a known module — code changes go through `Schedule`.
    /// `Origin::Module("governance") | System` only.
    Register { module_id: String, code_hash: Vec<u8> },
    /// schedule a height-gated code swap for a registered module.
    /// `Origin::Module | System` only.
    Schedule {
        name: String,
        module_id: String,
        activation_height: u64,
        code_hash: Vec<u8>,
    },
    /// schedule the ADMISSION of a brand-new module post-genesis: creates the
    /// entry with an EMPTY active hash and this pending, readiness-latched
    /// initial code. the module has no running code until the boundary
    /// realizes the swap; the host instantiates it from the fetched bytes at
    /// activation. cancelling before the boundary removes the entry entirely.
    /// `Origin::Module | System` only.
    ScheduleRegister {
        name: String,
        module_id: String,
        activation_height: u64,
        code_hash: Vec<u8>,
    },
    /// clear a pending swap before its boundary. `Origin::Module | System` only.
    Cancel { name: String, module_id: String },
    /// a validator records that it HOLDS (verified locally) the pending
    /// swap's component bytes. `Origin::External(validator)` only, member-
    /// gated against the valset; the last covering signal latches the swap
    /// `ready`.
    SignalReady { name: String, module_id: String },
    /// the system-injected boundary tick, keyed on `env.height`: activate every
    /// pending swap that is `ready` and whose `activation_height` has been
    /// reached. `Origin::System` only.
    Advance,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModregQuery {
    /// active + pending code for every registered module.
    Status,
    /// the swaps ARMED at `height` (`activation_height <= height`). the host reads
    /// this at the boundary to know which registry modules to swap and to which
    /// code hash.
    ArmedAt { height: u64 },
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModregReply {
    Status { modules: Vec<ModuleCode> },
    ArmedAt { swaps: Vec<ArmedSwap> },
}

pub fn encode_msg(m: &ModregMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<ModregMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &ModregQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<ModregQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &ModregReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<ModregReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
