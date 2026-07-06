//! the governance module's public wire surface — types only.
//!
//! governance is member-gated decision making over the validator set: a
//! CURRENT valset member proposes an action, members vote before a
//! consensus-time deadline, and anyone may trigger execution once the outcome
//! is decidable. passing membership actions are performed by emitting the
//! valset op as a host-drained follow-up — governance is the ONLY authorized
//! author of valset changes (the valset module rejects external submitters).
//!
//! authorship is trusted because the ordered lane VERIFIES frame signatures:
//! `Origin::External(pubkey)` reaching a module is authenticated, so a vote is
//! attributable to exactly one member key and no validator can forge another
//! member's ballot.

use serde::{Deserialize, Serialize};

/// what a passing proposal DOES.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum GovAction {
    /// admit a validator: emits `ValsetMsg::Join { key }` on execution.
    AddValidator { key: Vec<u8> },
    /// remove a validator: emits `ValsetMsg::Leave { key }` on execution.
    RemoveValidator { key: Vec<u8> },
    /// a binding signal with no on-chain effect beyond its recorded outcome.
    Signal { text: String },
    /// AUTHORIZE a pending node upgrade: emits `UpgradeMsg::Schedule { name,
    /// activation_height, to_version }` on execution. governance only SCHEDULES
    /// (authorizes) — it never ARMS: arming additionally requires the `R = n`
    /// readiness quorum evaluated by the upgrade module, and the upgrade module
    /// is the sole authority for the monotonicity / min-lead / at-most-one gates.
    ScheduleUpgrade {
        name: String,
        activation_height: u64,
        to_version: u32,
    },
    /// AUTHORIZE clearing a pending upgrade before its boundary: emits
    /// `UpgradeMsg::Cancel { name }` on execution.
    CancelUpgrade { name: String },
    /// grant OBSERVER standing (mesh + statesync, no quorum seat — the
    /// staged-admission tier): emits `ValsetMsg::Grant { key }` on execution.
    /// requires protocol version >= 3 (rejected at propose below it).
    AddObserver { key: Vec<u8> },
    /// revoke observer standing: emits `ValsetMsg::Revoke { key }` on
    /// execution. protocol version >= 3, like `AddObserver`.
    RemoveObserver { key: Vec<u8> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum GovMsg {
    /// open a proposal. the submitter (verified frame origin) must be a
    /// CURRENT valset member; the voting deadline is
    /// `consensus_time + voting_period`.
    Propose {
        proposal_id: String,
        action: GovAction,
        voting_period: u64,
    },
    /// cast (or change) the submitter's ballot while voting is open.
    Vote { proposal_id: String, approve: bool },
    /// tally and settle. anyone may trigger it once the outcome is decidable:
    /// after the deadline, or early once yes-ballots already form a strict
    /// majority of the CURRENT member count. passing membership actions emit
    /// their valset op as a follow-up in the same block.
    Execute { proposal_id: String },
}

/// a proposal's lifecycle. `Open` accepts votes; the rest are terminal.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalStatus {
    Open,
    Passed,
    Rejected,
}

/// the readable projection of one proposal.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ProposalView {
    pub proposal_id: String,
    pub action: GovAction,
    pub proposer: Vec<u8>,
    pub created_at: u64,
    pub deadline: u64,
    pub status: ProposalStatus,
    /// ballots by member key, sorted (BTreeMap on the impl side).
    pub votes: Vec<(Vec<u8>, bool)>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum GovQuery {
    /// every proposal, sorted by id.
    Proposals,
    /// one proposal by id.
    Proposal { proposal_id: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum GovReply {
    Proposals(Vec<ProposalView>),
    Proposal(Option<ProposalView>),
}

pub fn encode_msg(m: &GovMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<GovMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &GovQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<GovQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &GovReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<GovReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
