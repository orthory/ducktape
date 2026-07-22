//! validator governance with optional account-share voting.
//!
//! the module that closes the private network's membership loop: a CURRENT
//! valset member proposes (`AddValidator` / `RemoveValidator` / `Signal`),
//! members vote before a consensus-time deadline, and anyone may trigger
//! `Execute` once the outcome is decidable. a passing membership action emits
//! the valset op as a host-drained follow-up in the SAME block — and the
//! valset module only accepts membership ops from module origins, so
//! governance is the sole authorized author of membership change.
//!
//! ## why authorship is trustworthy
//!
//! the ordered lane verifies every frame's ed25519 signature before the host
//! sees it, so `Origin::External(pubkey)` here is AUTHENTICATED. validator mode
//! keys ballots directly; share mode deterministically resolves that node key
//! to its Identity account, so no validator can forge another principal's vote.
//!
//! ## determinism
//!
//! governance defaults to validator-node ballots. `AdoptShares` initializes an
//! explicit Identity-account allocation; governance may then switch future
//! proposals between validator and account principals. every new proposal
//! freezes its electorate and rule, so later mode/membership/share changes
//! cannot move its decision boundary or count the same shares twice.
//!
//! state model mirrors the tasks module: execute STAGES into a pending
//! overlay, `commit_block` publishes, `abort_block` discards; `root()` is
//! sha256 over the canonical encoding of COMMITTED proposals, and
//! `snapshot`/`install` ship exactly that root preimage (verify-then-adopt).

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;
// the invite capability: token types + verification, shared by the node's
// mint/lobby paths and the in-consensus `Redeem` handler below.
pub mod invite;

use std::collections::BTreeMap;

use commonware_codec::DecodeExt as _;
use commonware_cryptography::ed25519;
use identity::{
    IdentityMsg, IdentityQuery, IdentityReply, decode_reply as identity_decode_reply,
    encode_msg as identity_encode_msg, encode_query as identity_encode_query,
};
use sdk::codec::{Cursor, push_bytes};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};
use sha2::{Digest, Sha256};
use lifecycle::{LifecycleMsg, encode_msg as lifecycle_encode_msg};
use valset::{
    ValsetMsg, ValsetQuery, ValsetReply, decode_reply as valset_decode_reply,
    encode_msg as valset_encode_msg, encode_query as valset_encode_query,
};

/// ceiling on `voting_period` (in consensus-time units) — a fat-fingered or
/// hostile period must not park a proposal Open forever past any usable
/// horizon. views advance about once per finalized op, so this is generous.
const MAX_VOTING_PERIOD: u64 = 1_000_000_000;

/// Keep every share value and total exact in the JavaScript operator client.
const MAX_SAFE_SHARES: u64 = 9_007_199_254_740_991;
/// The v1 snapshot copies the complete electorate into each proposal. This is
/// intentionally the small-network implementation; checkpointed power history
/// replaces it if real deployments outgrow this bound.
const MAX_SHARE_ACCOUNTS: usize = 256;
/// Optional state extension marker. The initial/empty state (no frozen
/// electorate, no adopted share registry) and redemptions-only states omit it
/// and retain their bytes/root; the first frozen electorate or adopted share
/// registry appends this section.
const SHARES_EXT_MAGIC: &[u8; 8] = b"DGOVSHR1";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Proposal {
    action: GovAction,
    proposer: Vec<u8>,
    created_at: u64,
    deadline: u64,
    status: ProposalStatus,
    votes: BTreeMap<Vec<u8>, bool>,
    /// the electorate and rule frozen at `Propose` — every proposal has one, so
    /// later mode/membership/share changes cannot move its decision boundary.
    electorate: Electorate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Electorate {
    voter_kind: VoterKind,
    powers: BTreeMap<Vec<u8>, u64>,
    rule: VotingRule,
}

/// who a verified frame origin speaks for (see [`Governance::resolve_actor`]).
enum Actor {
    /// the origin is a member key of this Identity account: it acts for every
    /// node the account has bound (ballots stay NODE-keyed — one account
    /// owning three electorate nodes casts three node ballots, the exact
    /// power it held when each node voted for itself).
    Account {
        account_id: Vec<u8>,
        nodes: Vec<Vec<u8>>,
    },
    /// the origin is no account member — it acts as itself, a node key.
    Node(Vec<u8>),
}

impl Actor {
    /// the node keys this actor may cast ballots as / claim standing through.
    fn nodes(&self) -> &[Vec<u8>] {
        match self {
            Actor::Account { nodes, .. } => nodes,
            Actor::Node(node) => std::slice::from_ref(node),
        }
    }

    /// the id recorded as a proposal's `proposer` — stable across whichever
    /// member key of an account signed.
    fn principal(&self) -> &[u8] {
        match self {
            Actor::Account { account_id, .. } => account_id,
            Actor::Node(node) => node,
        }
    }
}

/// one settled invite redemption — the single-use record plus the audit
/// trail (who invited whom, when).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Redemption {
    joiner: Vec<u8>,
    issuer: Vec<u8>,
    height: u64,
}

pub struct Governance {
    id: ModuleId,
    /// the id of the valset module this governance instance gates. genesis
    /// wiring — identical on every node.
    valset_id: ModuleId,
    /// the id of the lifecycle module a passing `UpdateModule`/`CancelModuleUpdate`
    /// authorizes (the code-registry path — the same module, gated separately).
    /// genesis wiring — identical on every node; `None` (a net without the code
    /// registry wired) rejects those proposals at the door, deterministically.
    code_registry_id: Option<ModuleId>,
    /// the Identity account registry used in account-share mode AND as the
    /// submit-door client ACL: a redeemed `role=Client` invite emits an
    /// `IdentityMsg::GrantClient` follow-up here (identity is always wired, so
    /// Client redemption needs no separate module gate).
    identity_id: ModuleId,
    /// the network binding invite tokens sign over (the genesis namespace).
    /// genesis wiring — identical on every node of the same network. `None`
    /// (a shape without a descriptor) refuses every `Redeem` with a clear
    /// error, deterministically.
    invite_binding: Option<Vec<u8>>,
    /// committed proposals — what `root()` commits to.
    proposals: BTreeMap<String, Proposal>,
    /// this block's staged writes (whole-proposal overwrite granularity),
    /// read ahead of committed state, merged at `commit_block`.
    pending: BTreeMap<String, Proposal>,
    /// committed invite redemptions by token nonce — the exactly-once set.
    /// folded into `root()`/`snapshot()` after the proposal section.
    redeemed: BTreeMap<Vec<u8>, Redemption>,
    /// this block's staged redemptions, same discipline as `pending`.
    pending_redeemed: BTreeMap<Vec<u8>, Redemption>,
    /// `None` until the one-time AdoptShares action passes.
    shares: Option<BTreeMap<Vec<u8>, u64>>,
    /// this block's replacement share map, if a share action executed.
    pending_shares: Option<BTreeMap<Vec<u8>, u64>>,
    /// false by default: one validator node, one ballot.
    share_mode: bool,
    /// this block's mode switch, if one executed.
    pending_share_mode: Option<bool>,
}

impl Governance {
    pub fn new(
        id: impl Into<ModuleId>,
        valset_id: impl Into<ModuleId>,
        identity_id: impl Into<ModuleId>,
    ) -> Self {
        Self {
            id: id.into(),
            valset_id: valset_id.into(),
            code_registry_id: None,
            identity_id: identity_id.into(),
            invite_binding: None,
            proposals: BTreeMap::new(),
            pending: BTreeMap::new(),
            redeemed: BTreeMap::new(),
            pending_redeemed: BTreeMap::new(),
            shares: None,
            pending_shares: None,
            share_mode: false,
            pending_share_mode: None,
        }
    }

    /// enable the code-registry path (`UpdateModule`/`CancelModuleUpdate`) on the
    /// lifecycle module. genesis wiring — every node of a network must wire the
    /// same id (or none), or nodes diverge on whether those proposals are
    /// accepted.
    pub fn with_code_registry(mut self, id: impl Into<ModuleId>) -> Self {
        self.code_registry_id = Some(id.into());
        self
    }

    /// wire the network binding invite tokens verify against (the genesis
    /// namespace). every node of a network must wire the same bytes — a node
    /// without it rejects `Redeem` ops its peers accept, which forks.
    pub fn with_invite_binding(mut self, binding: impl Into<Vec<u8>>) -> Self {
        self.invite_binding = Some(binding.into());
        self
    }

    fn get(&self, id: &str) -> Option<&Proposal> {
        self.pending.get(id).or_else(|| self.proposals.get(id))
    }

    /// the AUTHENTICATED submitter key, or an error for module/system origins —
    /// proposals and ballots are member actions, never emitted by modules.
    fn external_origin(ctx: &dyn Ctx) -> Result<Vec<u8>, Error> {
        match &ctx.env().origin {
            Origin::External(key) => Ok(key.clone()),
            other => Err(Error::Module(format!(
                "governance actions require an external submitter, got {other:?}"
            ))),
        }
    }

    /// resolve the verified frame origin to a governance ACTOR (ADR A1, the
    /// account-signed lane): an origin that is a MEMBER KEY of an Identity
    /// account acts for that account — its ballots are the account's bound
    /// nodes. an origin that is no account member acts as ITSELF (a node key):
    /// a validator node remains a first-class governance actor (its own
    /// automation, upgrade tooling, self-serving ops), so this arm is the
    /// model, not a compat alias — the deleted thing is the node-local
    /// re-signing TRANSPORT, not node principals.
    async fn resolve_actor(&self, ctx: &dyn Ctx, origin: &[u8]) -> Result<Actor, Error> {
        // an origin that is no account member — including a network with no
        // identity module at all (an identity query error means the module is
        // absent, so no accounts exist) — acts as its own node key. a real
        // network always genesis-wires identity, so the error arm is only
        // reachable in the identity-less test hosts.
        let resolved = self
            .identity_account(
                ctx,
                IdentityQuery::OfMember {
                    member_key: origin.to_vec(),
                },
            )
            .await
            .unwrap_or(None);
        match resolved {
            Some(account) => Ok(Actor::Account {
                account_id: account.account_id,
                nodes: account.nodes.into_iter().map(|n| n.node_key).collect(),
            }),
            None => Ok(Actor::Node(origin.to_vec())),
        }
    }

    /// the Identity ACCOUNT a submitter speaks for in account (share) mode: a
    /// member key resolves to its own account; a node key resolves through its
    /// committed `BindNode`.
    async fn account_principal(&self, ctx: &dyn Ctx, submitter: &[u8]) -> Result<Vec<u8>, Error> {
        match self.resolve_actor(ctx, submitter).await? {
            Actor::Account { account_id, .. } => Ok(account_id),
            Actor::Node(node) => self.account_of_node(ctx, &node).await,
        }
    }

    /// the CURRENT member set: the valset module's staged-over-committed
    /// projection, via the shared `valset::members` read.
    async fn members(&self, ctx: &dyn Ctx) -> Result<Vec<Vec<u8>>, Error> {
        valset::members(ctx, &self.valset_id).await
    }

    fn effective_shares(&self) -> Option<&BTreeMap<Vec<u8>, u64>> {
        self.pending_shares.as_ref().or(self.shares.as_ref())
    }

    fn effective_share_mode(&self) -> bool {
        self.pending_share_mode.unwrap_or(self.share_mode)
    }

    async fn identity_account(
        &self,
        ctx: &dyn Ctx,
        query: IdentityQuery,
    ) -> Result<Option<identity::AccountView>, Error> {
        let reply = ctx
            .query(&self.identity_id, &identity_encode_query(&query))
            .await?;
        match identity_decode_reply(&reply).map_err(Error::Module)? {
            IdentityReply::Account(account) => Ok(account),
            other => Err(Error::Module(format!(
                "identity answered an account query with {other:?}"
            ))),
        }
    }

    async fn account_of_node(&self, ctx: &dyn Ctx, node_key: &[u8]) -> Result<Vec<u8>, Error> {
        self.identity_account(
            ctx,
            IdentityQuery::OfNode {
                node_key: node_key.to_vec(),
            },
        )
        .await?
        .map(|account| account.account_id)
        .ok_or_else(|| Error::Module("submitter node is not bound to an Identity account".into()))
    }

    async fn require_account(&self, ctx: &dyn Ctx, account_id: &[u8]) -> Result<(), Error> {
        if self
            .identity_account(
                ctx,
                IdentityQuery::Get {
                    account_id: account_id.to_vec(),
                },
            )
            .await?
            .is_some()
        {
            Ok(())
        } else {
            Err(Error::Module(
                "share allocation names no existing Identity account".into(),
            ))
        }
    }

    fn total_power(powers: &BTreeMap<Vec<u8>, u64>) -> Result<u64, Error> {
        let total = powers.values().try_fold(0u64, |sum, power| {
            sum.checked_add(*power)
                .ok_or_else(|| Error::Module("total governance shares overflow u64".into()))
        })?;
        if total == 0 || total > MAX_SAFE_SHARES {
            return Err(Error::Module(format!(
                "total governance shares must be in 1..={MAX_SAFE_SHARES}"
            )));
        }
        Ok(total)
    }

    fn threshold_rule(total: u64, action: &GovAction, share_mode: bool) -> VotingRule {
        if !share_mode {
            return VotingRule::Threshold {
                required_yes: total / 2 + 1,
            };
        }
        match action {
            GovAction::Signal { .. } => VotingRule::ParticipatingMajority {
                quorum: total / 2 + total % 2,
            },
            _ => VotingRule::Threshold {
                // ceil(2n/3), without multiplication overflow.
                required_yes: total - total / 3,
            },
        }
    }

    async fn frozen_electorate(
        &self,
        ctx: &dyn Ctx,
        submitter: &[u8],
        action: &GovAction,
    ) -> Result<(Vec<u8>, Electorate), Error> {
        if self.effective_share_mode() {
            let shares = self.effective_shares().ok_or_else(|| {
                Error::Module("account-share mode has no configured registry".into())
            })?;
            let account_id = self.account_principal(ctx, submitter).await?;
            if !shares.contains_key(&account_id) {
                return Err(Error::Module(
                    "submitter account holds no governance shares".into(),
                ));
            }
            let powers = shares.clone();
            let total = Self::total_power(&powers)?;
            return Ok((
                account_id,
                Electorate {
                    voter_kind: VoterKind::Account,
                    rule: Self::threshold_rule(total, action, true),
                    powers,
                },
            ));
        }

        // validator mode (the v1 default): ballots stay NODE-keyed — N
        // validators = N votes. the module-side ACL (A1): the submitter must
        // hold member standing. a submitter that is DIRECTLY a member node acts
        // as itself with NO identity read (a validator's own key, and any host
        // without an identity module); only a non-member origin is resolved as
        // an account member key through its committed `BindNode`s.
        let members = self.members(ctx).await?;
        let proposer = if members.iter().any(|member| member == submitter) {
            submitter.to_vec()
        } else {
            let actor = self.resolve_actor(ctx, submitter).await?;
            if !actor.nodes().iter().any(|node| members.contains(node)) {
                return Err(Error::Module(
                    "submitter holds no validator-set standing (no member node bound to it)"
                        .into(),
                ));
            }
            actor.principal().to_vec()
        };
        let powers: BTreeMap<Vec<u8>, u64> =
            members.into_iter().map(|member| (member, 1)).collect();
        let total = Self::total_power(&powers)?;
        Ok((
            proposer,
            Electorate {
                voter_kind: VoterKind::ValidatorNode,
                rule: Self::threshold_rule(total, action, false),
                powers,
            },
        ))
    }

    /// the node ballots one submitter casts against a NODE-keyed electorate: a
    /// submitter that is directly in the electorate casts its own (no identity
    /// read); otherwise it is resolved as an account member key and casts EVERY
    /// bound node still in the electorate. empty ⇒ the submitter has no standing.
    async fn node_ballots(
        &self,
        ctx: &dyn Ctx,
        submitter: &[u8],
        eligible: &dyn Fn(&[u8]) -> bool,
    ) -> Result<Vec<Vec<u8>>, Error> {
        if eligible(submitter) {
            return Ok(vec![submitter.to_vec()]);
        }
        let actor = self.resolve_actor(ctx, submitter).await?;
        Ok(actor
            .nodes()
            .iter()
            .filter(|node| eligible(node))
            .cloned()
            .collect())
    }

    /// the CURRENT resident set (valset's staged-over-committed projection) —
    /// the standing a redeemed joiner already holds.
    async fn residents(&self, ctx: &dyn Ctx) -> Result<Vec<Vec<u8>>, Error> {
        let reply = ctx
            .query(
                &self.valset_id,
                &valset_encode_query(&ValsetQuery::Residents),
            )
            .await?;
        match valset_decode_reply(&reply).map_err(Error::Module)? {
            ValsetReply::Residents(residents) => Ok(residents),
            other => Err(Error::Module(format!(
                "valset answered a Residents query with {other:?}"
            ))),
        }
    }

    fn view_of(id: &str, p: &Proposal) -> ProposalView {
        let electorate = &p.electorate;
        ProposalView {
            proposal_id: id.to_string(),
            action: p.action.clone(),
            proposer: p.proposer.clone(),
            created_at: p.created_at,
            deadline: p.deadline,
            status: p.status,
            votes: p.votes.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            voter_kind: electorate.voter_kind,
            electorate: electorate.powers.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            voting_rule: electorate.rule,
        }
    }

    fn shares_view(&self) -> SharesView {
        let Some(shares) = self.effective_shares() else {
            return SharesView {
                active: false,
                allocations: Vec::new(),
                total: 0,
            };
        };
        SharesView {
            active: self.effective_share_mode(),
            allocations: shares
                .iter()
                .map(|(account_id, shares)| ShareAllocation {
                    account_id: account_id.clone(),
                    shares: *shares,
                })
                .collect(),
            total: shares.values().sum(),
        }
    }

    // ---- canonical state bytes (root preimage + snapshot format) -----------

    fn encode_state(
        proposals: &BTreeMap<String, Proposal>,
        redeemed: &BTreeMap<Vec<u8>, Redemption>,
        shares: Option<&BTreeMap<Vec<u8>, u64>>,
        share_mode: bool,
    ) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(proposals.len() as u64).to_le_bytes());
        for (id, p) in proposals {
            push_bytes(&mut out, id.as_bytes());
            match &p.action {
                GovAction::AddValidator { key } => {
                    out.push(0);
                    push_bytes(&mut out, key);
                }
                GovAction::RemoveValidator { key } => {
                    out.push(1);
                    push_bytes(&mut out, key);
                }
                GovAction::Signal { text } => {
                    out.push(2);
                    push_bytes(&mut out, text.as_bytes());
                }
                GovAction::AddResident { key } => {
                    out.push(3);
                    push_bytes(&mut out, key);
                }
                GovAction::RemoveResident { key } => {
                    out.push(4);
                    push_bytes(&mut out, key);
                }
                GovAction::AdoptShares { allocations } => {
                    out.push(5);
                    out.extend_from_slice(&(allocations.len() as u64).to_le_bytes());
                    for allocation in allocations {
                        push_bytes(&mut out, &allocation.account_id);
                        out.extend_from_slice(&allocation.shares.to_le_bytes());
                    }
                }
                GovAction::SetShares { account_id, shares } => {
                    out.push(6);
                    push_bytes(&mut out, account_id);
                    out.extend_from_slice(&shares.to_le_bytes());
                }
                GovAction::SetShareMode { enabled } => {
                    out.push(7);
                    out.push(u8::from(*enabled));
                }
                GovAction::UpdateModule {
                    name,
                    module_id,
                    activation_height,
                    code_hash,
                } => {
                    out.push(8);
                    push_bytes(&mut out, name.as_bytes());
                    push_bytes(&mut out, module_id.as_bytes());
                    out.extend_from_slice(&activation_height.to_le_bytes());
                    push_bytes(&mut out, code_hash);
                }
                GovAction::CancelModuleUpdate { name, module_id } => {
                    out.push(9);
                    push_bytes(&mut out, name.as_bytes());
                    push_bytes(&mut out, module_id.as_bytes());
                }
                GovAction::RegisterModule {
                    name,
                    module_id,
                    activation_height,
                    code_hash,
                } => {
                    out.push(10);
                    push_bytes(&mut out, name.as_bytes());
                    push_bytes(&mut out, module_id.as_bytes());
                    out.extend_from_slice(&activation_height.to_le_bytes());
                    push_bytes(&mut out, code_hash);
                }
            }
            push_bytes(&mut out, &p.proposer);
            out.extend_from_slice(&p.created_at.to_le_bytes());
            out.extend_from_slice(&p.deadline.to_le_bytes());
            out.push(match p.status {
                ProposalStatus::Open => 0,
                ProposalStatus::Passed => 1,
                ProposalStatus::Rejected => 2,
            });
            out.extend_from_slice(&(p.votes.len() as u64).to_le_bytes());
            for (voter, approve) in &p.votes {
                push_bytes(&mut out, voter);
                out.push(u8::from(*approve));
            }
        }
        // the redemption section: count, then per sorted nonce its record.
        out.extend_from_slice(&(redeemed.len() as u64).to_le_bytes());
        for (nonce, r) in redeemed {
            push_bytes(&mut out, nonce);
            push_bytes(&mut out, &r.joiner);
            push_bytes(&mut out, &r.issuer);
            out.extend_from_slice(&r.height.to_le_bytes());
        }
        // every proposal freezes its electorate, so the extension carries one
        // record per proposal. it rides whenever there is anything to persist:
        // a proposal (hence its electorate), a share registry, or a mode flag.
        if shares.is_some() || !proposals.is_empty() || share_mode {
            out.extend_from_slice(SHARES_EXT_MAGIC);
            out.push(1); // extension version
            match shares {
                Some(shares) => {
                    out.push(1);
                    out.extend_from_slice(&(shares.len() as u64).to_le_bytes());
                    for (account_id, power) in shares {
                        push_bytes(&mut out, account_id);
                        out.extend_from_slice(&power.to_le_bytes());
                    }
                }
                None => out.push(0),
            }
            out.extend_from_slice(&(proposals.len() as u64).to_le_bytes());
            for (id, proposal) in proposals {
                let electorate = &proposal.electorate;
                push_bytes(&mut out, id.as_bytes());
                out.push(match electorate.voter_kind {
                    VoterKind::ValidatorNode => 0,
                    VoterKind::Account => 1,
                });
                match electorate.rule {
                    VotingRule::Threshold { required_yes } => {
                        out.push(1);
                        out.extend_from_slice(&required_yes.to_le_bytes());
                    }
                    VotingRule::ParticipatingMajority { quorum } => {
                        out.push(2);
                        out.extend_from_slice(&quorum.to_le_bytes());
                    }
                }
                out.extend_from_slice(&(electorate.powers.len() as u64).to_le_bytes());
                for (principal, power) in &electorate.powers {
                    push_bytes(&mut out, principal);
                    out.extend_from_slice(&power.to_le_bytes());
                }
            }
            // The original extension implied share mode from registry
            // presence. Preserve those exact bytes/roots; append one override
            // byte only after governance switches a configured registry off.
            if share_mode != shares.is_some() {
                out.push(u8::from(share_mode));
            }
        }
        out
    }

    fn root_of(
        proposals: &BTreeMap<String, Proposal>,
        redeemed: &BTreeMap<Vec<u8>, Redemption>,
        shares: Option<&BTreeMap<Vec<u8>, u64>>,
        share_mode: bool,
    ) -> StateRoot {
        if proposals.is_empty() && redeemed.is_empty() && shares.is_none() && !share_mode {
            return StateRoot::ZERO;
        }
        let mut h = Sha256::new();
        h.update(Self::encode_state(proposals, redeemed, shares, share_mode));
        StateRoot(h.finalize().into())
    }

    /// canonical bytes of COMMITTED state — the exact preimage of `root()`.
    pub fn snapshot(&self) -> Vec<u8> {
        Self::encode_state(
            &self.proposals,
            &self.redeemed,
            self.shares.as_ref(),
            self.share_mode,
        )
    }

    /// verify-then-adopt a peer snapshot: decode into a temporary, recompute
    /// the root, refuse on mismatch — committed state and stage untouched on
    /// any error. success drops the stage (it belonged to the replaced state).
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let (proposals, redeemed, shares, share_mode) = decode_state(bytes)?;
        sdk::verify_snapshot_root(Self::root_of(&proposals, &redeemed, shares.as_ref(), share_mode), expected)?;
        self.proposals = proposals;
        self.redeemed = redeemed;
        self.shares = shares;
        self.share_mode = share_mode;
        self.pending.clear();
        self.pending_redeemed.clear();
        self.pending_shares = None;
        self.pending_share_mode = None;
        Ok(())
    }

    async fn normalize_share_action(
        &self,
        ctx: &dyn Ctx,
        mut action: GovAction,
    ) -> Result<GovAction, Error> {
        match &mut action {
            GovAction::AdoptShares { allocations } => {
                if self.effective_shares().is_some() {
                    return Err(Error::Module(
                        "governance shares are already configured".into(),
                    ));
                }
                if allocations.is_empty() || allocations.len() > MAX_SHARE_ACCOUNTS {
                    return Err(Error::Module(format!(
                        "initial share allocation must contain 1..={MAX_SHARE_ACCOUNTS} accounts"
                    )));
                }
                let mut normalized = BTreeMap::new();
                for allocation in std::mem::take(allocations) {
                    if allocation.shares == 0 {
                        return Err(Error::Module(
                            "initial share allocations must be positive".into(),
                        ));
                    }
                    self.require_account(ctx, &allocation.account_id).await?;
                    if normalized
                        .insert(allocation.account_id, allocation.shares)
                        .is_some()
                    {
                        return Err(Error::Module(
                            "initial share allocation contains a duplicate account".into(),
                        ));
                    }
                }
                Self::total_power(&normalized)?;
                *allocations = normalized
                    .into_iter()
                    .map(|(account_id, shares)| ShareAllocation { account_id, shares })
                    .collect();
            }
            GovAction::SetShares { account_id, shares } => {
                let current = self.effective_shares().ok_or_else(|| {
                    Error::Module("adopt governance shares before changing them".into())
                })?;
                if *shares > MAX_SAFE_SHARES {
                    return Err(Error::Module(format!(
                        "account shares must be at most {MAX_SAFE_SHARES}"
                    )));
                }
                self.require_account(ctx, account_id).await?;
                let mut after = current.clone();
                if *shares == 0 {
                    after.remove(account_id);
                } else {
                    after.insert(account_id.clone(), *shares);
                }
                if after.len() > MAX_SHARE_ACCOUNTS {
                    return Err(Error::Module(format!(
                        "share registry supports at most {MAX_SHARE_ACCOUNTS} accounts"
                    )));
                }
                Self::total_power(&after)?;
            }
            GovAction::SetShareMode { enabled } => {
                if *enabled && self.effective_shares().is_none() {
                    return Err(Error::Module(
                        "configure governance shares before enabling share mode".into(),
                    ));
                }
                if *enabled == self.effective_share_mode() {
                    return Err(Error::Module(
                        "governance is already using the requested voting mode".into(),
                    ));
                }
            }
            _ => {}
        }
        Ok(action)
    }

    // ---- the op handlers ----------------------------------------------------

    async fn handle_propose(
        &mut self,
        ctx: &mut dyn Ctx,
        proposal_id: String,
        action: GovAction,
        voting_period: u64,
    ) -> Result<(), Error> {
        if proposal_id.is_empty() {
            return Err(Error::Module("proposal_id must not be empty".into()));
        }
        if voting_period == 0 || voting_period > MAX_VOTING_PERIOD {
            return Err(Error::Module(format!(
                "voting_period must be in 1..={MAX_VOTING_PERIOD}"
            )));
        }
        if let GovAction::AddValidator { key }
        | GovAction::RemoveValidator { key }
        | GovAction::AddResident { key }
        | GovAction::RemoveResident { key } = &action
        {
            // shape-check the key here so a proposal that can never execute is
            // rejected at the door, not at tally time.
            if key.len() != 32 {
                return Err(Error::Module(
                    "membership key must be a 32-byte ed25519 public key".into(),
                ));
            }
        }
        // module-update authorizations: shape-checked at the door (a proposal
        // that can never execute is rejected here, not at tally time); the code
        // registry's min-lead / at-most-one / no-op gates are NOT duplicated —
        // the lifecycle module is their sole authority at ingest. a net without a
        // wired code registry deterministically rejects these (genesis wiring is
        // identical on every node).
        if let GovAction::UpdateModule { name, module_id, .. }
        | GovAction::RegisterModule { name, module_id, .. }
        | GovAction::CancelModuleUpdate { name, module_id } = &action
        {
            if self.code_registry_id.is_none() {
                return Err(Error::Module(
                    "no code registry wired: module updates are not available on this network"
                        .into(),
                ));
            }
            if name.is_empty() {
                return Err(Error::Module("module update name must not be empty".into()));
            }
            if module_id.is_empty() {
                return Err(Error::Module("module_id must not be empty".into()));
            }
        }
        if let GovAction::UpdateModule { code_hash, .. }
        | GovAction::RegisterModule { code_hash, .. } = &action
            && code_hash.len() != lifecycle::CODE_HASH_LEN
        {
            return Err(Error::Module(format!(
                "code_hash must be {} bytes (sha256 of the component)",
                lifecycle::CODE_HASH_LEN
            )));
        }
        if self.get(&proposal_id).is_some() {
            return Err(Error::Module(format!(
                "proposal already exists: {proposal_id}"
            )));
        }
        let submitter = Self::external_origin(ctx)?;
        let (proposer, electorate) = self.frozen_electorate(ctx, &submitter, &action).await?;
        // Gate the submitter before resolving up to MAX_SHARE_ACCOUNTS Identity
        // records for an adoption proposal.
        let action = self.normalize_share_action(ctx, action).await?;

        let now = ctx.env().consensus_time;
        let deadline = now
            .checked_add(voting_period)
            .ok_or_else(|| Error::Module("voting deadline overflows consensus time".into()))?;
        self.pending.insert(
            proposal_id,
            Proposal {
                action,
                proposer,
                created_at: now,
                deadline,
                status: ProposalStatus::Open,
                votes: BTreeMap::new(),
                electorate,
            },
        );
        Ok(())
    }

    async fn handle_vote(
        &mut self,
        ctx: &mut dyn Ctx,
        proposal_id: String,
        approve: bool,
    ) -> Result<(), Error> {
        let mut proposal = self
            .get(&proposal_id)
            .cloned()
            .ok_or_else(|| Error::Module(format!("no such proposal: {proposal_id}")))?;
        if proposal.status != ProposalStatus::Open {
            return Err(Error::Module("proposal is settled".into()));
        }
        if ctx.env().consensus_time >= proposal.deadline {
            return Err(Error::Module("voting closed at the deadline".into()));
        }
        let submitter = Self::external_origin(ctx)?;
        let electorate = &proposal.electorate;
        // the ballots this op casts, by the proposal's frozen principal kind.
        let voters: Vec<Vec<u8>> = match electorate.voter_kind {
            // node-keyed ballots (N validators = N votes): a submitter in the
            // frozen electorate casts its own; an account member's op casts
            // EVERY bound node still in the electorate — the same power the
            // account held when each node voted for itself.
            VoterKind::ValidatorNode => {
                let voters = self
                    .node_ballots(ctx, &submitter, &|node| electorate.powers.contains_key(node))
                    .await?;
                if voters.is_empty() {
                    return Err(Error::Module(
                        "submitter is not a member of this proposal's frozen electorate".into(),
                    ));
                }
                voters
            }
            VoterKind::Account => {
                let principal = self.account_principal(ctx, &submitter).await?;
                if !electorate.powers.contains_key(&principal) {
                    return Err(Error::Module(
                        "submitter is not a member of this proposal's frozen electorate".into(),
                    ));
                }
                vec![principal]
            }
        };
        // Re-voting overwrites by frozen principal. Two nodes bound to one
        // account therefore cast the same-direction ballots together, and an
        // account (share) principal stays one ballot regardless of node count.
        for voter in voters {
            proposal.votes.insert(voter, approve);
        }
        self.pending.insert(proposal_id, proposal);
        Ok(())
    }

    async fn handle_execute(
        &mut self,
        ctx: &mut dyn Ctx,
        proposal_id: String,
    ) -> Result<(), Error> {
        let mut proposal = self
            .get(&proposal_id)
            .cloned()
            .ok_or_else(|| Error::Module(format!("no such proposal: {proposal_id}")))?;
        if proposal.status != ProposalStatus::Open {
            return Err(Error::Module("proposal is settled".into()));
        }

        let electorate = &proposal.electorate;
        let mut yes = 0u64;
        let mut no = 0u64;
        for (principal, power) in &electorate.powers {
            match proposal.votes.get(principal) {
                Some(true) => yes += power,
                Some(false) => no += power,
                None => {}
            }
        }
        let total = Self::total_power(&electorate.powers)?;
        let rule = electorate.rule;

        let (passes, decidable_early) = match rule {
            VotingRule::Threshold { required_yes } => (yes >= required_yes, yes >= required_yes),
            VotingRule::ParticipatingMajority { quorum } => {
                let participation = yes + no;
                let passes = participation >= quorum && yes > no;
                // Safe early passage only when every still-uncast ballot could
                // vote no and yes would remain the strict majority.
                let irreversible = participation >= quorum && yes > total - yes;
                (passes, irreversible)
            }
        };
        if ctx.env().consensus_time < proposal.deadline && !decidable_early {
            return Err(Error::Module(format!(
                "not decidable yet: voting open until {} (yes={yes}, no={no}, total={total})",
                proposal.deadline
            )));
        }

        if passes {
            proposal.status = ProposalStatus::Passed;
            // a passing membership action is PERFORMED by emitting the valset
            // op as a follow-up — the host drains it in this same block, and
            // valset accepts it because the origin is Module(governance).
            match &proposal.action {
                GovAction::AddValidator { key } => ctx.emit_msg(Msg {
                    target: self.valset_id.clone(),
                    payload: valset_encode_msg(&ValsetMsg::Join { key: key.clone() }),
                }),
                GovAction::RemoveValidator { key } => {
                    // never enact a removal that would empty the validator set: a
                    // zero-validator orderer hits commonware `quorum(0)`, which
                    // panics. the valset Leave handler enforces this invariant
                    // authoritatively (returning an Err that would abort the WHOLE
                    // block), so we pre-check here and cleanly REJECT the proposal
                    // instead — the happy path never emits a set-emptying Leave.
                    let members = self.members(ctx).await?;
                    if members.iter().all(|m| m == key) {
                        proposal.status = ProposalStatus::Rejected;
                    } else {
                        ctx.emit_msg(Msg {
                            target: self.valset_id.clone(),
                            payload: valset_encode_msg(&ValsetMsg::Leave { key: key.clone() }),
                        });
                    }
                }
                // a passing module-update authorization is PERFORMED the same
                // way: emit the modreg op as a follow-up, accepted because the
                // origin is Module(governance). governance only authorizes; the
                // code registry owns the min-lead / at-most-one / no-op gates,
                // and the swap arms purely on height. Propose door-checks the
                // wiring, so an unwired registry here means the proposal was
                // adopted from a peer snapshot minted under DIFFERENT genesis
                // wiring — reject it cleanly rather than emit into the void.
                GovAction::UpdateModule {
                    name,
                    module_id,
                    activation_height,
                    code_hash,
                } => match &self.code_registry_id {
                    Some(lifecycle) => ctx.emit_msg(Msg {
                        target: lifecycle.clone(),
                        payload: lifecycle_encode_msg(&LifecycleMsg::ScheduleSwap {
                            name: name.clone(),
                            module_id: module_id.clone(),
                            activation_height: *activation_height,
                            code_hash: code_hash.clone(),
                        }),
                    }),
                    None => proposal.status = ProposalStatus::Rejected,
                },
                // a passing ADMISSION is performed the same way; the code
                // registry owns the not-already-registered / min-lead gates and
                // the R=n readiness quorum is what arms it.
                GovAction::RegisterModule {
                    name,
                    module_id,
                    activation_height,
                    code_hash,
                } => match &self.code_registry_id {
                    Some(lifecycle) => ctx.emit_msg(Msg {
                        target: lifecycle.clone(),
                        payload: lifecycle_encode_msg(&LifecycleMsg::ScheduleRegister {
                            name: name.clone(),
                            module_id: module_id.clone(),
                            activation_height: *activation_height,
                            code_hash: code_hash.clone(),
                        }),
                    }),
                    None => proposal.status = ProposalStatus::Rejected,
                },
                GovAction::CancelModuleUpdate { name, module_id } => match &self.code_registry_id {
                    Some(lifecycle) => ctx.emit_msg(Msg {
                        target: lifecycle.clone(),
                        payload: lifecycle_encode_msg(&LifecycleMsg::CancelSwap {
                            name: name.clone(),
                            module_id: module_id.clone(),
                        }),
                    }),
                    None => proposal.status = ProposalStatus::Rejected,
                },
                // the staged-admission grant/revoke: valset owns the
                // validator-overlap rule.
                GovAction::AddResident { key } => ctx.emit_msg(Msg {
                    target: self.valset_id.clone(),
                    payload: valset_encode_msg(&ValsetMsg::Grant { key: key.clone() }),
                }),
                GovAction::RemoveResident { key } => ctx.emit_msg(Msg {
                    target: self.valset_id.clone(),
                    payload: valset_encode_msg(&ValsetMsg::Revoke { key: key.clone() }),
                }),
                GovAction::AdoptShares { allocations } => {
                    if self.effective_shares().is_some() {
                        // A competing adoption may have won since this proposal
                        // opened. Settle cleanly; initialization is one-time.
                        proposal.status = ProposalStatus::Rejected;
                    } else {
                        let shares: BTreeMap<Vec<u8>, u64> = allocations
                            .iter()
                            .map(|a| (a.account_id.clone(), a.shares))
                            .collect();
                        if Self::total_power(&shares).is_err() {
                            proposal.status = ProposalStatus::Rejected;
                        } else {
                            self.pending_shares = Some(shares);
                            self.pending_share_mode = Some(true);
                        }
                    }
                }
                GovAction::SetShares { account_id, shares } => {
                    let Some(mut after) = self.effective_shares().cloned() else {
                        proposal.status = ProposalStatus::Rejected;
                        self.pending.insert(proposal_id, proposal);
                        return Ok(());
                    };
                    if *shares == 0 {
                        after.remove(account_id);
                    } else {
                        after.insert(account_id.clone(), *shares);
                    }
                    if after.len() > MAX_SHARE_ACCOUNTS || Self::total_power(&after).is_err() {
                        proposal.status = ProposalStatus::Rejected;
                    } else {
                        self.pending_shares = Some(after);
                    }
                }
                GovAction::SetShareMode { enabled } => {
                    if *enabled && self.effective_shares().is_none() {
                        proposal.status = ProposalStatus::Rejected;
                    } else {
                        self.pending_share_mode = Some(*enabled);
                    }
                }
                GovAction::Signal { .. } => {}
            }
        } else {
            proposal.status = ProposalStatus::Rejected;
        }
        self.pending.insert(proposal_id, proposal);
        Ok(())
    }

    /// redeem an invite — no ballot, the mint WAS the admission decision.
    /// verification is fully in-consensus so every validator settles the op
    /// identically: token signature and join proof against the wired binding,
    /// issuer against CURRENT membership, nonce against the redeemed set
    /// (single-use — a second redemption of the same token deterministically
    /// rejects). success emits the observer grant in the same block.
    #[allow(clippy::too_many_arguments)]
    async fn handle_redeem(
        &mut self,
        ctx: &mut dyn Ctx,
        issuer: Vec<u8>,
        nonce: Vec<u8>,
        token_sig: Vec<u8>,
        joiner: Vec<u8>,
        proof: Vec<u8>,
        role: u8,
        expires_unix_secs: u64,
    ) -> Result<(), Error> {
        // the submitter must be an authenticated frame origin, but is NOT
        // required to be a member — the token authorizes the admission, not
        // the relaying node.
        Self::external_origin(ctx)?;
        let Some(binding) = self.invite_binding.as_deref() else {
            return Err(Error::Module(
                "this network is not wired for invite redemption (no binding)".into(),
            ));
        };
        let issuer_key = ed25519::PublicKey::decode(issuer.as_slice())
            .map_err(|e| Error::Module(format!("issuer key: {e}")))?;
        let joiner_key = ed25519::PublicKey::decode(joiner.as_slice())
            .map_err(|e| Error::Module(format!("joiner key: {e}")))?;
        if nonce.len() != invite::INVITE_NONCE_LEN {
            return Err(Error::Module(format!(
                "nonce must be {} bytes",
                invite::INVITE_NONCE_LEN
            )));
        }
        let mut nonce_arr = [0u8; invite::INVITE_NONCE_LEN];
        nonce_arr.copy_from_slice(&nonce);
        let sig = ed25519::Signature::decode(token_sig.as_slice())
            .map_err(|e| Error::Module(format!("token signature: {e}")))?;
        let proof_sig = ed25519::Signature::decode(proof.as_slice())
            .map_err(|e| Error::Module(format!("join proof: {e}")))?;
        let role = invite::InviteRole::from_u8(role).map_err(Error::Module)?;
        // EVERY invite is bearer (기명 dropped — see the join ADR): there is
        // no target lock. The join proof below binds the redemption to
        // whichever key presents it, and the nonce set makes that
        // exactly-once — that is the whole containment story.
        let token = invite::InviteToken {
            issuer: issuer_key,
            nonce: nonce_arr,
            role,
            expires_unix_secs,
            sig,
        };
        if !invite::verify_invite_token(&token, binding) {
            return Err(Error::Module(
                "invite token signature does not verify for this network".into(),
            ));
        }
        // expiry is NOT enforced here: `consensus_time` is block height on
        // this chain, so no deterministic wall clock exists in-consensus.
        // enforcement lives at the joiner's decode and at every gating
        // member's wall clock before it submits this Redeem (lobby + intro
        // doorbells), and single-use bounds any residual window. the field
        // stays in the op because it is signature-covered — members need it
        // to check expiry against the same bytes the issuer signed.
        if !invite::verify_join_proof(&joiner_key, binding, &token, &proof_sig) {
            return Err(Error::Module(
                "joiner proof-of-possession does not verify".into(),
            ));
        }
        // a removed member's outstanding invites die with it.
        let members = self.members(ctx).await?;
        if !members.iter().any(|m| m == &issuer) {
            return Err(Error::Module(
                "the inviting member is no longer part of this network".into(),
            ));
        }
        // the standing grant differs by role: a Resident invite grants valset
        // resident standing (mesh + statesync, pre-promotion); a Client invite
        // grants client-ACL standing — SUBMIT AUTHORIZATION ONLY, never
        // statesync or a quorum seat (client standing is a facet of identity,
        // structurally distinct from valset so the sync door never reads it).
        // the dedup gate and the emitted follow-up op are role-specific; every
        // check above is shared.
        let grant = match token.role {
            invite::InviteRole::Resident => {
                if members.iter().any(|m| m == &joiner) {
                    return Err(Error::Module("joiner is already a validator".into()));
                }
                if self.residents(ctx).await?.iter().any(|o| o == &joiner) {
                    return Err(Error::Module(
                        "joiner already holds resident standing".into(),
                    ));
                }
                Msg {
                    target: self.valset_id.clone(),
                    payload: valset_encode_msg(&ValsetMsg::Grant {
                        key: joiner.clone(),
                    }),
                }
            }
            invite::InviteRole::Client => {
                // client standing is a facet of the identity account plane
                // (identity is always wired), so redemption needs no separate
                // module gate — it emits an `IdentityMsg::GrantClient` follow-up.
                if identity::clients(ctx, &self.identity_id)
                    .await?
                    .iter()
                    .any(|c| c == &joiner)
                {
                    return Err(Error::Module("joiner already holds client standing".into()));
                }
                Msg {
                    target: self.identity_id.to_string(),
                    payload: identity_encode_msg(&IdentityMsg::GrantClient {
                        key: joiner.clone(),
                    }),
                }
            }
        };
        // exactly-once: the nonce is the single-use key, SHARED across roles
        // (pending-over-committed read, so two redemptions in one block settle
        // first-wins too). a Client and a Resident invite carry different
        // nonces, but the set is shared so neither token can be replayed as the
        // other's.
        if self.pending_redeemed.contains_key(&nonce) || self.redeemed.contains_key(&nonce) {
            return Err(Error::Module("invite already redeemed".into()));
        }
        self.pending_redeemed.insert(
            nonce,
            Redemption {
                joiner,
                issuer,
                height: ctx.env().height,
            },
        );
        ctx.emit_msg(grant);
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Governance {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// sha256 over the canonical encoding of COMMITTED proposals and
    /// redemptions; `ZERO` when none exist (the sdk's uninitialized-module
    /// sentinel).
    fn root(&self) -> StateRoot {
        Self::root_of(
            &self.proposals,
            &self.redeemed,
            self.shares.as_ref(),
            self.share_mode,
        )
    }

    fn snapshot_bytes(&self) -> Option<Vec<u8>> {
        Some(self.snapshot())
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            GovMsg::Propose {
                proposal_id,
                action,
                voting_period,
            } => {
                self.handle_propose(ctx, proposal_id, action, voting_period)
                    .await
            }
            GovMsg::Vote {
                proposal_id,
                approve,
            } => self.handle_vote(ctx, proposal_id, approve).await,
            GovMsg::Execute { proposal_id } => self.handle_execute(ctx, proposal_id).await,
            GovMsg::Redeem {
                issuer,
                nonce,
                token_sig,
                joiner,
                proof,
                role,
                expires_unix_secs,
            } => {
                self.handle_redeem(ctx, issuer, nonce, token_sig, joiner, proof, role, expires_unix_secs)
                    .await
            }
        }
    }

    /// read projection — committed plus this block's staged changes.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            GovQuery::Proposals => {
                let mut merged = self.proposals.clone();
                for (id, p) in &self.pending {
                    merged.insert(id.clone(), p.clone());
                }
                let views = merged.iter().map(|(id, p)| Self::view_of(id, p)).collect();
                Ok(encode_reply(&GovReply::Proposals(views)))
            }
            GovQuery::Proposal { proposal_id } => Ok(encode_reply(&GovReply::Proposal(
                self.get(&proposal_id)
                    .map(|p| Self::view_of(&proposal_id, p)),
            ))),
            GovQuery::Redemptions => {
                let mut merged = self.redeemed.clone();
                for (nonce, r) in &self.pending_redeemed {
                    merged.insert(nonce.clone(), r.clone());
                }
                let views = merged
                    .iter()
                    .map(|(nonce, r)| RedemptionView {
                        nonce: nonce.clone(),
                        joiner: r.joiner.clone(),
                        issuer: r.issuer.clone(),
                        height: r.height,
                    })
                    .collect();
                Ok(encode_reply(&GovReply::Redemptions(views)))
            }
            GovQuery::Shares => Ok(encode_reply(&GovReply::Shares(self.shares_view()))),
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (id, p) in std::mem::take(&mut self.pending) {
            self.proposals.insert(id, p);
        }
        for (nonce, r) in std::mem::take(&mut self.pending_redeemed) {
            self.redeemed.insert(nonce, r);
        }
        if let Some(shares) = self.pending_shares.take() {
            self.shares = Some(shares);
        }
        if let Some(share_mode) = self.pending_share_mode.take() {
            self.share_mode = share_mode;
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        self.pending_redeemed.clear();
        self.pending_shares = None;
        self.pending_share_mode = None;
        Ok(())
    }
}

// ---- strict snapshot decode (untrusted bytes) -------------------------------

type DecodedState = (
    BTreeMap<String, Proposal>,
    BTreeMap<Vec<u8>, Redemption>,
    Option<BTreeMap<Vec<u8>, u64>>,
    bool,
);

/// a proposal decoded from the snapshot's proposal section — everything but the
/// frozen electorate, which rides the later share-extension section. every
/// proposal must be paired with exactly one electorate before it becomes a
/// `Proposal`; one the extension does not name is refused (no network freezes a
/// proposal without an electorate).
struct PendingProposal {
    action: GovAction,
    proposer: Vec<u8>,
    created_at: u64,
    deadline: u64,
    status: ProposalStatus,
    votes: BTreeMap<Vec<u8>, bool>,
}

fn decode_state(bytes: &[u8]) -> Result<DecodedState, Error> {
    let mut cur = Cursor::new(bytes);
    let count = cur.u64("snapshot proposal count")?;
    // every proposal costs at least its id length prefix — a forged count can
    // never drive allocation past the buffer.
    cur.bound(count, 8, "snapshot proposal")?;
    // phase 1: the proposal section, sans electorate (see `PendingProposal`).
    let mut pending: BTreeMap<String, PendingProposal> = BTreeMap::new();
    let mut prev_id: Option<String> = None;
    for _ in 0..count {
        let id = cur.string("snapshot proposal id")?;
        // strictly increasing ids: one state has exactly one encoding.
        if prev_id.as_deref().is_some_and(|p| p >= id.as_str()) {
            return Err(Error::Module(
                "snapshot proposal ids must be strictly increasing".into(),
            ));
        }
        let action = match cur.byte("snapshot action tag")? {
            0 => GovAction::AddValidator {
                key: cur.bytes("snapshot key")?.to_vec(),
            },
            1 => GovAction::RemoveValidator {
                key: cur.bytes("snapshot key")?.to_vec(),
            },
            2 => GovAction::Signal {
                text: cur.string("snapshot signal text")?,
            },
            3 => GovAction::AddResident {
                key: cur.bytes("snapshot key")?.to_vec(),
            },
            4 => GovAction::RemoveResident {
                key: cur.bytes("snapshot key")?.to_vec(),
            },
            5 => {
                let allocation_count = cur.u64("snapshot allocation count")?;
                if allocation_count == 0 || allocation_count > MAX_SHARE_ACCOUNTS as u64 {
                    return Err(Error::Module(
                        "snapshot initial shares have an invalid account count".into(),
                    ));
                }
                let mut allocations = Vec::with_capacity(allocation_count as usize);
                let mut powers = BTreeMap::new();
                for _ in 0..allocation_count {
                    let account_id = cur.bytes("snapshot allocation account")?.to_vec();
                    let shares = cur.u64("snapshot allocation shares")?;
                    if shares == 0 || powers.insert(account_id.clone(), shares).is_some() {
                        return Err(Error::Module(
                            "snapshot initial shares are zero or duplicated".into(),
                        ));
                    }
                    allocations.push(ShareAllocation { account_id, shares });
                }
                if allocations
                    .windows(2)
                    .any(|pair| pair[0].account_id >= pair[1].account_id)
                {
                    return Err(Error::Module(
                        "snapshot initial shares must be strictly increasing".into(),
                    ));
                }
                Governance::total_power(&powers)?;
                GovAction::AdoptShares { allocations }
            }
            6 => GovAction::SetShares {
                account_id: cur.bytes("snapshot account id")?.to_vec(),
                shares: cur.u64("snapshot shares")?,
            },
            7 => GovAction::SetShareMode {
                enabled: cur.bool("snapshot share-mode action")?,
            },
            8 => GovAction::UpdateModule {
                name: cur.string("snapshot module update name")?,
                module_id: cur.string("snapshot module id")?,
                activation_height: cur.u64("snapshot activation height")?,
                code_hash: cur.bytes("snapshot code hash")?.to_vec(),
            },
            9 => GovAction::CancelModuleUpdate {
                name: cur.string("snapshot module update name")?,
                module_id: cur.string("snapshot module id")?,
            },
            10 => GovAction::RegisterModule {
                name: cur.string("snapshot module update name")?,
                module_id: cur.string("snapshot module id")?,
                activation_height: cur.u64("snapshot activation height")?,
                code_hash: cur.bytes("snapshot code hash")?.to_vec(),
            },
            other => return Err(Error::Module(format!("snapshot: bad action tag {other}"))),
        };
        let proposer = cur.bytes("snapshot proposer")?.to_vec();
        let created_at = cur.u64("snapshot created_at")?;
        let deadline = cur.u64("snapshot deadline")?;
        let status = match cur.byte("snapshot status tag")? {
            0 => ProposalStatus::Open,
            1 => ProposalStatus::Passed,
            2 => ProposalStatus::Rejected,
            other => return Err(Error::Module(format!("snapshot: bad status tag {other}"))),
        };
        let vote_count = cur.u64("snapshot vote count")?;
        cur.bound(vote_count, 9, "snapshot vote")?;
        let mut votes = BTreeMap::new();
        let mut prev_voter: Option<Vec<u8>> = None;
        for _ in 0..vote_count {
            let voter = cur.bytes("snapshot voter")?.to_vec();
            if prev_voter.as_deref().is_some_and(|p| p >= voter.as_slice()) {
                return Err(Error::Module(
                    "snapshot voters must be strictly increasing".into(),
                ));
            }
            let approve = cur.bool("snapshot ballot")?;
            prev_voter = Some(voter.clone());
            votes.insert(voter, approve);
        }
        prev_id = Some(id.clone());
        pending.insert(
            id,
            PendingProposal {
                action,
                proposer,
                created_at,
                deadline,
                status,
                votes,
            },
        );
    }
    // the redemption section always rides, then the optional share extension.
    let rcount = cur.u64("snapshot redemption count")?;
    cur.bound(rcount, 8, "snapshot redemption")?;
    let mut redeemed = BTreeMap::new();
    let mut prev_nonce: Option<Vec<u8>> = None;
    for _ in 0..rcount {
        let nonce = cur.bytes("snapshot redemption nonce")?.to_vec();
        if prev_nonce.as_deref().is_some_and(|p| p >= nonce.as_slice()) {
            return Err(Error::Module(
                "snapshot redemption nonces must be strictly increasing".into(),
            ));
        }
        let joiner = cur.bytes("snapshot redemption joiner")?.to_vec();
        let issuer = cur.bytes("snapshot redemption issuer")?.to_vec();
        let height = cur.u64("snapshot redemption height")?;
        prev_nonce = Some(nonce.clone());
        redeemed.insert(
            nonce,
            Redemption {
                joiner,
                issuer,
                height,
            },
        );
    }

    // phase 2: the share extension carries the registry, the mode flag, and
    // EVERY proposal's frozen electorate. an absent extension is legal only
    // when there is nothing to carry (no proposals, no shares, no share mode).
    let mut electorates: BTreeMap<String, Electorate> = BTreeMap::new();
    let (shares, share_mode) = if cur.remaining() == 0 {
        (None, false)
    } else {
        if cur.array::<8>("snapshot share-extension magic")? != *SHARES_EXT_MAGIC {
            return Err(Error::Module("snapshot carries trailing bytes".into()));
        }
        if cur.byte("snapshot share-extension version")? != 1 {
            return Err(Error::Module(
                "snapshot has an unsupported governance-share extension".into(),
            ));
        }
        let shares = match cur.byte("snapshot share-active flag")? {
            0 => None,
            1 => {
                let share_count = cur.u64("snapshot share count")?;
                if share_count == 0 || share_count > MAX_SHARE_ACCOUNTS as u64 {
                    return Err(Error::Module(
                        "snapshot share registry has an invalid account count".into(),
                    ));
                }
                let mut map = BTreeMap::new();
                let mut previous: Option<Vec<u8>> = None;
                for _ in 0..share_count {
                    let account_id = cur.bytes("snapshot share account")?.to_vec();
                    if previous
                        .as_deref()
                        .is_some_and(|prev| prev >= account_id.as_slice())
                    {
                        return Err(Error::Module(
                            "snapshot share accounts must be strictly increasing".into(),
                        ));
                    }
                    let power = cur.u64("snapshot share power")?;
                    if power == 0 {
                        return Err(Error::Module(
                            "snapshot share power must be positive".into(),
                        ));
                    }
                    previous = Some(account_id.clone());
                    map.insert(account_id, power);
                }
                Governance::total_power(&map)?;
                Some(map)
            }
            other => {
                return Err(Error::Module(format!(
                    "snapshot share-active flag must be 0 or 1, got {other}"
                )));
            }
        };

        let meta_count = cur.u64("snapshot electorate count")?;
        if meta_count > pending.len() as u64 {
            return Err(Error::Module(
                "snapshot has more electorates than proposals".into(),
            ));
        }
        let mut previous_id: Option<String> = None;
        for _ in 0..meta_count {
            let id = cur.string("snapshot electorate id")?;
            if previous_id
                .as_deref()
                .is_some_and(|prev| prev >= id.as_str())
            {
                return Err(Error::Module(
                    "snapshot electorate ids must be strictly increasing".into(),
                ));
            }
            let voter_kind = match cur.byte("snapshot voter-kind tag")? {
                0 => VoterKind::ValidatorNode,
                1 => VoterKind::Account,
                other => {
                    return Err(Error::Module(format!(
                        "snapshot has bad voter-kind tag {other}"
                    )));
                }
            };
            // tag 0 is retired with the dynamic-validator rule: it no longer
            // names a rule, so it is refused here like any other bad tag.
            let rule = match cur.byte("snapshot voting-rule tag")? {
                1 => VotingRule::Threshold {
                    required_yes: cur.u64("snapshot required_yes")?,
                },
                2 => VotingRule::ParticipatingMajority {
                    quorum: cur.u64("snapshot quorum")?,
                },
                other => {
                    return Err(Error::Module(format!(
                        "snapshot has bad voting-rule tag {other}"
                    )));
                }
            };
            let power_count = cur.u64("snapshot electorate power count")?;
            if power_count == 0 {
                return Err(Error::Module(
                    "snapshot electorate count exceeds buffer".into(),
                ));
            }
            cur.bound(power_count, 16, "snapshot electorate power")?;
            let mut powers = BTreeMap::new();
            let mut previous_principal: Option<Vec<u8>> = None;
            for _ in 0..power_count {
                let principal = cur.bytes("snapshot electorate principal")?.to_vec();
                if previous_principal
                    .as_deref()
                    .is_some_and(|prev| prev >= principal.as_slice())
                {
                    return Err(Error::Module(
                        "snapshot electorate must be strictly increasing".into(),
                    ));
                }
                let power = cur.u64("snapshot electorate power")?;
                if power == 0 {
                    return Err(Error::Module(
                        "snapshot electorate power must be positive".into(),
                    ));
                }
                previous_principal = Some(principal.clone());
                powers.insert(principal, power);
            }
            let total = Governance::total_power(&powers)?;
            match rule {
                VotingRule::Threshold { required_yes }
                    if required_yes == 0 || required_yes > total =>
                {
                    return Err(Error::Module(
                        "snapshot threshold is outside electorate power".into(),
                    ));
                }
                VotingRule::ParticipatingMajority { quorum } if quorum == 0 || quorum > total => {
                    return Err(Error::Module(
                        "snapshot quorum is outside electorate power".into(),
                    ));
                }
                _ => {}
            }
            let proposal = pending
                .get(&id)
                .ok_or_else(|| Error::Module("snapshot electorate names no proposal".into()))?;
            if proposal
                .votes
                .keys()
                .any(|principal| !powers.contains_key(principal))
            {
                return Err(Error::Module(
                    "snapshot ballot is outside its frozen electorate".into(),
                ));
            }
            electorates.insert(
                id.clone(),
                Electorate {
                    voter_kind,
                    powers,
                    rule,
                },
            );
            previous_id = Some(id);
        }
        let share_mode = if cur.remaining() == 0 {
            // Compatibility with the original extension: registry presence
            // implied that account shares governed future proposals.
            shares.is_some()
        } else {
            cur.bool("snapshot share-mode override")?
        };
        if share_mode && shares.is_none() {
            return Err(Error::Module(
                "snapshot enables share mode without a registry".into(),
            ));
        }
        (shares, share_mode)
    };
    cur.finish("snapshot")?;
    // assemble: pair every proposal with its frozen electorate. there is no
    // dynamic-electorate fallback — a proposal the extension did not name is
    // refused here (the retired `electorate: None` shape).
    let mut proposals = BTreeMap::new();
    for (id, p) in pending {
        let electorate = electorates
            .remove(&id)
            .ok_or_else(|| Error::Module("snapshot proposal has no frozen electorate".into()))?;
        proposals.insert(
            id,
            Proposal {
                action: p.action,
                proposer: p.proposer,
                created_at: p.created_at,
                deadline: p.deadline,
                status: p.status,
                votes: p.votes,
                electorate,
            },
        );
    }
    Ok((proposals, redeemed, shares, share_mode))
}

#[cfg(test)]
mod dead_electorate_refusal {
    //! the retired dynamic-validator path — `electorate: None` /
    //! `VotingRule::DynamicValidatorMajority` — must be UNREACHABLE. these craft
    //! the two shapes the old decoder tolerated and prove the new one refuses
    //! both, so no snapshot can resurrect the dynamic electorate.
    use super::*;

    /// the proposal section without the share extension that carries frozen
    /// electorates: the pre-share shape that used to decode to `electorate:
    /// None`. it must be refused now — every proposal freezes an electorate.
    fn one_proposal_no_extension() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u64.to_le_bytes()); // one proposal
        push_bytes(&mut bytes, b"p"); // proposal id
        bytes.push(2); // Signal action tag
        push_bytes(&mut bytes, b"x"); // signal text
        push_bytes(&mut bytes, &[0u8; 32]); // proposer
        bytes.extend_from_slice(&0u64.to_le_bytes()); // created_at
        bytes.extend_from_slice(&0u64.to_le_bytes()); // deadline
        bytes.push(0); // status Open
        bytes.extend_from_slice(&0u64.to_le_bytes()); // vote count
        bytes.extend_from_slice(&0u64.to_le_bytes()); // redemption count
        bytes
    }

    #[test]
    fn a_proposal_without_a_frozen_electorate_is_refused() {
        let err = decode_state(&one_proposal_no_extension())
            .expect_err("a proposal with no frozen electorate must not decode");
        assert!(
            matches!(&err, Error::Module(m) if m.contains("no frozen electorate")),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn the_retired_dynamic_voting_rule_tag_is_refused() {
        // the same proposal, this time WITH an extension that tags its frozen
        // electorate with the retired dynamic rule (tag 0). the tag no longer
        // names a rule, so decode refuses it before reading powers.
        let mut bytes = one_proposal_no_extension();
        bytes.extend_from_slice(SHARES_EXT_MAGIC); // extension magic
        bytes.push(1); // extension version
        bytes.push(0); // share-active flag: no registry
        bytes.extend_from_slice(&1u64.to_le_bytes()); // one electorate record
        push_bytes(&mut bytes, b"p"); // names the proposal
        bytes.push(0); // voter_kind ValidatorNode
        bytes.push(0); // voting-rule tag 0 — the retired dynamic rule
        let err = decode_state(&bytes)
            .expect_err("the retired dynamic voting-rule tag must not decode");
        assert!(
            matches!(&err, Error::Module(m) if m.contains("bad voting-rule tag")),
            "unexpected error: {err:?}"
        );
    }
}

// the wasm-guest port: the dispatch shell that adapts this module to the
// ducktape:module world. compiled only by the guest-builder's synthesized
// wasm32 cdylib workspace (feature `guest`), never by the native build.
#[cfg(feature = "guest")]
mod guest;
