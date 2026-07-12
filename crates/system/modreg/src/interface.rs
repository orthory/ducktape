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

/// coordinates of a scheduled code swap for one module. **at most one** is ever
/// pending per module.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ScheduledSwap {
    pub name: String,
    pub activation_height: u64,
    /// the 32-byte sha256 of the target component bytes.
    pub code_hash: Vec<u8>,
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
    /// clear a pending swap before its boundary. `Origin::Module | System` only.
    Cancel { name: String, module_id: String },
    /// the system-injected boundary tick, keyed on `env.height`: activate every
    /// pending swap whose `activation_height` has been reached. `Origin::System`
    /// only.
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
