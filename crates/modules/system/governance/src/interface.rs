//! the governance module's public wire surface — types only.
//!
//! governance defaults to one ballot per validator node. the validators may
//! initialize non-transferable Identity-account shares, then governance can
//! switch future proposals between validator and account-share mode. every new
//! proposal freezes the electorate that decides it. passing
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
    /// AUTHORIZE a pending node upgrade: emits `LifecycleMsg::ScheduleUpgrade {
    /// name, activation_height, to_version }` on execution. governance only SCHEDULES
    /// (authorizes) — it never ARMS: arming additionally requires the `R = n`
    /// readiness quorum evaluated by the lifecycle module, and the lifecycle module
    /// is the sole authority for the monotonicity / min-lead / at-most-one gates.
    ScheduleUpgrade {
        name: String,
        activation_height: u64,
        to_version: u32,
    },
    /// AUTHORIZE clearing a pending upgrade before its boundary: emits
    /// `LifecycleMsg::CancelUpgrade { name }` on execution.
    CancelUpgrade { name: String },
    /// grant RESIDENT standing (mesh + statesync, no quorum seat — the
    /// staged-admission tier): emits `ValsetMsg::Grant { key }` on execution.
    AddResident { key: Vec<u8> },
    /// revoke resident standing: emits `ValsetMsg::Revoke { key }` on
    /// execution.
    RemoveResident { key: Vec<u8> },
    /// initialize non-transferable account shares and enable share mode. the
    /// allocation must be non-empty, unique by account id, and name existing
    /// Identity accounts. initialization is one-time; the registry is retained
    /// when governance later switches back to validator ballots.
    AdoptShares { allocations: Vec<ShareAllocation> },
    /// set one Identity account's shares. zero removes the account from the
    /// registry. only available after shares were configured; the current mode
    /// freezes the electorate and structural threshold that decides it.
    SetShares { account_id: Vec<u8>, shares: u64 },
    /// choose the electorate for future proposals. `true` uses the configured
    /// account shares; `false` restores one ballot per validator node. this
    /// proposal itself is always decided by the mode frozen when it was opened.
    SetShareMode { enabled: bool },
    /// AUTHORIZE a height-gated wasm module code swap: emits `LifecycleMsg::
    /// ScheduleSwap { name, module_id, activation_height, code_hash }` on execution.
    /// governance only authorizes — the code registry is the sole authority for
    /// the min-lead / at-most-one / no-op-swap gates, and the swap arms purely
    /// on height (the bytes travel out-of-band, content-addressed by the hash).
    UpdateModule {
        name: String,
        module_id: String,
        activation_height: u64,
        /// sha256 of the target component bytes (32 bytes).
        code_hash: Vec<u8>,
    },
    /// AUTHORIZE the post-genesis ADMISSION of a brand-new wasm module: emits
    /// `LifecycleMsg::ScheduleRegister { name, module_id, activation_height,
    /// code_hash }` on execution. governance only authorizes — the code
    /// registry owns the not-already-registered / min-lead gates, the R = n
    /// readiness quorum arms it, and the host instantiates the module from the
    /// content-addressed bytes at the activation boundary. cancellation before
    /// the boundary rides `CancelModuleUpdate` (an admission cancel removes
    /// the registry entry entirely).
    RegisterModule {
        name: String,
        module_id: String,
        activation_height: u64,
        /// sha256 of the initial component bytes (32 bytes).
        code_hash: Vec<u8>,
    },
    /// AUTHORIZE clearing a pending module code swap before its boundary: emits
    /// `LifecycleMsg::CancelSwap { name, module_id }` on execution.
    CancelModuleUpdate { name: String, module_id: String },
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
    /// Pass once `yes_power >= required_yes`; this covers validator-set
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
    /// electorate selected by the current governance mode.
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
    /// via the redeemed-nonce set in consensus state. success emits the
    /// role's grant in the same block: `Resident` → `ValsetMsg::Grant`
    /// (full node standing — mesh + statesync, no quorum seat), `Client` →
    /// `IdentityMsg::GrantClient` (submit authorization only).
    Redeem {
        issuer: Vec<u8>,
        nonce: Vec<u8>,
        token_sig: Vec<u8>,
        joiner: Vec<u8>,
        proof: Vec<u8>,
        /// the invite role byte (`Resident = 0`, `Client = 1`). a `Resident`
        /// redeem grants valset resident standing; a `Client` redeem grants
        /// submit-only client standing in the identity module. EVERY invite
        /// is bearer (기명 dropped in Join Protocol v2): no `target` — the
        /// `proof` binds the redemption to whichever key presents it and the
        /// nonce set makes that exactly-once.
        role: u8,
        /// the token's unix-seconds expiry — enforced against block time.
        expires_unix_secs: u64,
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
    /// the proposal-time power snapshot — every proposal freezes its electorate
    /// when it is opened.
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
    /// the current share registry and whether it governs new proposals.
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
