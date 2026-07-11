//! the governance module's public wire surface — types only.
//!
//! governance starts as member-gated decision making over the validator set.
//! the validators may make a one-way [`GovAction::AdoptShares`] transition to
//! account governance: non-transferable shares are held by Identity account id,
//! and every new proposal freezes the electorate that decides it. passing
//! membership actions are performed by emitting the valset op as a host-drained
//! follow-up — governance remains the ONLY authorized author of valset changes.
//!
//! authorship is trusted because the ordered lane VERIFIES frame signatures:
//! `Origin::External(pubkey)` reaching a module is authenticated. validator
//! mode keys a ballot by that node; share mode resolves it to one Identity
//! account, so no validator can forge another principal's ballot.

use serde::{Deserialize, Serialize};

/// what a passing proposal DOES.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
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
    /// grant RESIDENT standing (mesh + statesync, no quorum seat — the
    /// staged-admission tier): emits `ValsetMsg::Grant { key }` on execution.
    AddResident { key: Vec<u8> },
    /// revoke resident standing: emits `ValsetMsg::Revoke { key }` on
    /// execution.
    RemoveResident { key: Vec<u8> },
    /// one-time transition from validator-node ballots to non-transferable
    /// account shares. the allocation must be non-empty, unique by account id,
    /// and name existing Identity accounts. this proposal is decided by the
    /// legacy validator electorate that opened it; activation is one-way.
    AdoptShares { allocations: Vec<ShareAllocation> },
    /// set one Identity account's shares. zero removes the account from the
    /// electorate. only available after shares were adopted, and therefore
    /// decided by a frozen two-thirds share threshold like every structural
    /// action.
    SetShares { account_id: Vec<u8>, shares: u64 },
}

/// one non-transferable governance-share allocation.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ShareAllocation {
    pub account_id: Vec<u8>,
    pub shares: u64,
}

/// what principal the proposal's ballot keys identify.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VoterKind {
    ValidatorNode,
    Account,
}

/// the exact decision rule frozen with a proposal.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VotingRule {
    /// Compatibility for proposals restored from the pre-share snapshot shape:
    /// use the validator set current at execution, exactly as the old binary did.
    DynamicValidatorMajority,
    /// Pass once `yes_power >= required_yes`; this covers legacy validator
    /// snapshots and structural proposals requiring two thirds of all shares.
    Threshold { required_yes: u64 },
    /// At the deadline, participation must reach `quorum` and yes must exceed
    /// no. Early passage is allowed only when no remaining ballot can reverse it.
    ParticipatingMajority { quorum: u64 },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GovMsg {
    /// open a proposal. the verified submitter must belong to the current
    /// validator electorate before adoption, or hold account shares after it.
    /// the voting deadline is `consensus_time + voting_period`.
    Propose {
        proposal_id: String,
        action: GovAction,
        voting_period: u64,
    },
    /// cast (or change) the submitter's ballot while voting is open.
    Vote { proposal_id: String, approve: bool },
    /// tally and settle. anyone may trigger it after the deadline, or early
    /// once the proposal's frozen rule can no longer be reversed. passing
    /// membership actions emit their valset op as a follow-up in the same block.
    Execute { proposal_id: String },
    /// redeem an invite: no ballot — MINTING was the admission decision. the
    /// op carries the token's fields plus the joiner key and its
    /// proof-of-possession (all raw bytes, mirroring the lobby announce);
    /// the module re-verifies both signatures against the network binding,
    /// requires the issuer to be a CURRENT member, and enforces single-use
    /// via the redeemed-nonce set in consensus state. success emits
    /// `ValsetMsg::Grant { key: joiner }` in the same block — the joiner
    /// becomes a full node (mesh + statesync standing, no quorum seat).
    Redeem {
        issuer: Vec<u8>,
        nonce: Vec<u8>,
        token_sig: Vec<u8>,
        joiner: Vec<u8>,
        proof: Vec<u8>,
    },
}

/// a proposal's lifecycle. `Open` accepts votes; the rest are terminal.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
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
    /// ballots by validator-node key or account id, per `voter_kind`.
    pub votes: Vec<(Vec<u8>, bool)>,
    pub voter_kind: VoterKind,
    /// the proposal-time power snapshot. empty only for a proposal restored
    /// from the legacy dynamic-validator snapshot format.
    pub electorate: Vec<(Vec<u8>, u64)>,
    pub voting_rule: VotingRule,
}

/// the current non-transferable share registry.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SharesView {
    pub active: bool,
    pub allocations: Vec<ShareAllocation>,
    pub total: u64,
}

/// the readable projection of one settled invite redemption — the audit
/// trail of who invited whom.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RedemptionView {
    /// the redeemed token's nonce (the single-use key).
    pub nonce: Vec<u8>,
    /// the admitted key.
    pub joiner: Vec<u8>,
    /// the member whose token authorized the admission.
    pub issuer: Vec<u8>,
    /// the block height the redemption landed at.
    pub height: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GovQuery {
    /// every proposal, sorted by id.
    Proposals,
    /// one proposal by id.
    Proposal { proposal_id: String },
    /// every settled invite redemption, sorted by nonce.
    Redemptions,
    /// the current share registry; inactive and empty before adoption.
    Shares,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GovReply {
    Proposals(Vec<ProposalView>),
    Proposal(Option<ProposalView>),
    Redemptions(Vec<RedemptionView>),
    Shares(SharesView),
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
