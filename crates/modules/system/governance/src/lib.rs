//! qmdb-backed validator governance with optional account-share voting.
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
//! ## State model
//!
//! pure logic over a host-injected [`sdk::MerkleStore`]: the HOST constructs
//! the concrete store (qmdb today — `statesync::qmdb::QmdbStore`) and hands it
//! to [`Governance::new`], so this crate never names a storage crate. one
//! logical record per proposal (`prop\0{id}`) and per settled invite
//! redemption (`red\0{nonce}`), plus three aggregate records:
//!
//! - the proposal ROSTER (the sorted proposal-id list, bounded by
//!   [`MAX_PROPOSALS`]) — the ONE enumeration read. it stays canonical
//!   because governance's read model CANNOT move to the derived index tier:
//!   a proposal's frozen electorate, every ballot's principal resolution,
//!   and the settlement tally all read the valset/identity SIBLINGS at
//!   execute time, so an index fold over governance's own applied ops could
//!   only reproduce proposal state by re-implementing the consensus tally
//!   over other modules' state — a second consensus implementation, which is
//!   worse than a bounded canonical id list. the operator ceremonies (the
//!   CLI's adopt-an-open-proposal flow) consume this listing;
//! - the SHARE REGISTRY (bounded by [`MAX_SHARE_ACCOUNTS`]) — consensus
//!   consumes it whenever a proposal freezes an account electorate;
//! - the share-MODE flag — consensus consumes it on every `Propose`.
//!
//! redemptions have NO enumeration: the exactly-once gate and the node's
//! join-lobby pre-check (V6) are both point reads by nonce, so the set lives
//! as point records alone and grows without an aggregate to poison.
//!
//! writes are staged during a block and flushed to the store in one batch at
//! `commit_block`; the module root IS the store's merkle root. sync belongs
//! to the store, not this module: a joiner rebuilds the concrete store from a
//! peer (`QmdbStore::sync_from`) and wraps a fresh `Governance` around it.
//!
//! oversized values never reach the store (the poison-value lesson — the qmdb
//! wire codec bounds a value at decode, so an over-cap committed value would
//! wedge every syncing peer): a NEW proposal record is byte-gated at half of
//! [`MAX_PROPOSAL_RECORD_BYTES`] (accumulated ballots can at most double the
//! frozen-electorate section, so a settled record stays under the full cap),
//! the roster is byte-gated at [`MAX_ROSTER_RECORD_BYTES`] on top of its
//! id-length and count caps, and a redemption record is fixed-size by
//! construction (32-byte keys, 16-byte nonce).
//!
//! ## Genesis config (the invite binding)
//!
//! the per-network invite binding reaches the NATIVE module through
//! [`Governance::with_invite_binding`]. the wasm tenant is fixed bytes, so
//! there the binding rides GENESIS CONFIG: the host seeds the reserved
//! `__config` entry ([`sdk::genesis_config`]) into this module's store at
//! genesis construction — under [`sdk::store_key`], the same logical→store
//! mapping every record here uses — and the guest decodes it per dispatch.
//! the config is consensus state in the store's merkle root from genesis and
//! rides state-sync like any other record. this module never writes that key.

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
use lifecycle::{LifecycleMsg, encode_msg as lifecycle_encode_msg};
use sdk::{
    Ctx, Error, MerkleStore, Module, ModuleId, Msg, Origin, ResolverSyncTarget, StagedStore,
    StateRoot, StateSyncHandle,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
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
/// The frozen electorate copies the complete allocation into each proposal.
/// This is intentionally the small-network implementation; checkpointed power
/// history replaces it if real deployments outgrow this bound.
const MAX_SHARE_ACCOUNTS: usize = 256;
/// `proposal_id` byte bound — roster arithmetic and record keys need ids that
/// cannot balloon.
pub const MAX_PROPOSAL_ID_BYTES: usize = 256;
/// proposals retained over the network's life (settled proposals keep their
/// ids forever). proposing past this is refused loudly at execute.
pub const MAX_PROPOSALS: usize = 1024;
/// serialized roster-record byte bound, enforced at propose. the id-length and
/// count caps do not bound the roster's SERIALIZED form tightly enough on
/// their own: [`MAX_PROPOSALS`] ids of [`MAX_PROPOSAL_ID_BYTES`] control
/// characters JSON-escape past the qmdb wire codec's decode ceiling — a
/// committed over-cap value would wedge every syncing peer (the poison-value
/// lesson), so the propose op refuses loudly instead.
pub const MAX_ROSTER_RECORD_BYTES: usize = 512 * 1024;
/// serialized proposal-record ceiling. a NEW proposal is gated at HALF this
/// value: ballots accrue only from principals inside the frozen electorate
/// and a ballot entry is no larger than its electorate entry, so a fully
/// voted record is at most twice its at-propose size — the settled record
/// can never cross the full cap (which itself sits under the qmdb 1 MiB
/// value-decode ceiling).
pub const MAX_PROPOSAL_RECORD_BYTES: usize = 512 * 1024;

/// per-proposal record key: prefix + 0 + id (the single-component shape chat
/// uses). safe because every key literal below is fixed and none is another
/// followed by a 0 byte.
fn prop_key(proposal_id: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(4 + 1 + proposal_id.len());
    key.extend_from_slice(b"prop");
    key.push(0);
    key.extend_from_slice(proposal_id.as_bytes());
    key
}

/// per-redemption record key: prefix + 0 + nonce (16 raw bytes).
fn red_key(nonce: &[u8]) -> Vec<u8> {
    let mut key = Vec::with_capacity(3 + 1 + nonce.len());
    key.extend_from_slice(b"red");
    key.push(0);
    key.extend_from_slice(nonce);
    key
}

/// the proposal roster's whole key. collides with no `prop\0...`/`red\0...`
/// key (nor the host-seeded `__config` genesis-config record).
const PROPOSAL_ROSTER_KEY: &[u8] = b"proposals";

/// the share registry's whole key. present = shares were configured.
const SHARES_KEY: &[u8] = b"shares";

/// the share-mode flag's whole key. absent = validator ballots (the default).
const SHARE_MODE_KEY: &[u8] = b"mode";

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

/// the stored form of a [`Proposal`]: maps flatten to sorted pair lists
/// because the record codec (JSON) cannot key an object by raw bytes.
/// `BTreeMap` iteration writes them sorted; `collect` rebuilds the maps.
#[derive(Serialize, Deserialize)]
struct ProposalRecord {
    action: GovAction,
    proposer: Vec<u8>,
    created_at: u64,
    deadline: u64,
    status: ProposalStatus,
    votes: Vec<(Vec<u8>, bool)>,
    voter_kind: VoterKind,
    electorate: Vec<(Vec<u8>, u64)>,
    rule: VotingRule,
}

impl From<&Proposal> for ProposalRecord {
    fn from(p: &Proposal) -> Self {
        Self {
            action: p.action.clone(),
            proposer: p.proposer.clone(),
            created_at: p.created_at,
            deadline: p.deadline,
            status: p.status,
            votes: p.votes.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            voter_kind: p.electorate.voter_kind,
            electorate: p
                .electorate
                .powers
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
            rule: p.electorate.rule,
        }
    }
}

impl From<ProposalRecord> for Proposal {
    fn from(r: ProposalRecord) -> Self {
        Self {
            action: r.action,
            proposer: r.proposer,
            created_at: r.created_at,
            deadline: r.deadline,
            status: r.status,
            votes: r.votes.into_iter().collect(),
            electorate: Electorate {
                voter_kind: r.voter_kind,
                powers: r.electorate.into_iter().collect(),
                rule: r.rule,
            },
        }
    }
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
/// trail (who invited whom, when). the nonce is the record KEY, not a field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// the host-injected authenticated store plus this block's staging overlay
    /// (read-your-writes, folded into `root()` at `commit_block`). store key
    /// is `sha256(logical_key)`, owned by [`StagedStore`].
    staged: StagedStore,
}

impl Governance {
    /// wrap the host-constructed store under module identity `id`.
    pub fn new(
        id: impl Into<ModuleId>,
        store: Box<dyn MerkleStore>,
        valset_id: impl Into<ModuleId>,
        identity_id: impl Into<ModuleId>,
    ) -> Self {
        Self {
            id: id.into(),
            valset_id: valset_id.into(),
            code_registry_id: None,
            identity_id: identity_id.into(),
            invite_binding: None,
            staged: StagedStore::new(store),
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

    // ---- staged-over-committed reads ----------------------------------------

    async fn load<T>(&self, key: &[u8]) -> Result<Option<T>, Error>
    where
        T: DeserializeOwned,
    {
        match self.staged.get(key).await? {
            Some(bytes) => Ok(Some(
                serde_json::from_slice(&bytes).map_err(|e| Error::Module(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }

    /// stage a value whose serialized size is bounded by construction (a
    /// redemption record, the share registry, the mode flag) — see the module
    /// doc's poison-value paragraph. proposals and the roster go through
    /// [`Self::store_bounded`].
    fn store<T>(&mut self, key: Vec<u8>, value: &T)
    where
        T: Serialize,
    {
        self.staged.stage(
            key,
            serde_json::to_vec(value).expect("governance value is serializable"),
        );
    }

    /// stage a value only if its serialized size fits `cap` — the write-time
    /// guard against poison values (the qmdb codec cap is decode-only).
    fn store_bounded<T>(
        &mut self,
        key: Vec<u8>,
        value: &T,
        cap: usize,
        what: &str,
    ) -> Result<(), Error>
    where
        T: Serialize,
    {
        let bytes = serde_json::to_vec(value).expect("governance value is serializable");
        if bytes.len() > cap {
            return Err(Error::Module(format!(
                "{what} record too large: {} > {cap} bytes",
                bytes.len()
            )));
        }
        self.staged.stage(key, bytes);
        Ok(())
    }

    async fn proposal(&self, proposal_id: &str) -> Result<Option<Proposal>, Error> {
        Ok(self
            .load::<ProposalRecord>(&prop_key(proposal_id))
            .await?
            .map(Proposal::from))
    }

    /// stage a settled/updated proposal record under the FULL byte cap — see
    /// [`MAX_PROPOSAL_RECORD_BYTES`] for why accrued ballots cannot cross it.
    fn store_proposal(&mut self, proposal_id: &str, proposal: &Proposal) -> Result<(), Error> {
        self.store_bounded(
            prop_key(proposal_id),
            &ProposalRecord::from(proposal),
            MAX_PROPOSAL_RECORD_BYTES,
            "proposal",
        )
    }

    /// the proposal roster — every proposal id, sorted. record and roster are
    /// staged (and commit or abort) together, so membership in one is
    /// membership in both; the roster is the ONE existence authority at
    /// propose and the ONE enumeration read (module doc: why it is canonical).
    async fn roster(&self) -> Result<Vec<String>, Error> {
        Ok(self.load(PROPOSAL_ROSTER_KEY).await?.unwrap_or_default())
    }

    async fn shares(&self) -> Result<Option<BTreeMap<Vec<u8>, u64>>, Error> {
        Ok(self
            .load::<Vec<ShareAllocation>>(SHARES_KEY)
            .await?
            .map(|allocations| {
                allocations
                    .into_iter()
                    .map(|a| (a.account_id, a.shares))
                    .collect()
            }))
    }

    fn stage_shares(&mut self, shares: &BTreeMap<Vec<u8>, u64>) {
        let allocations: Vec<ShareAllocation> = shares
            .iter()
            .map(|(account_id, shares)| ShareAllocation {
                account_id: account_id.clone(),
                shares: *shares,
            })
            .collect();
        // bounded by construction: at most MAX_SHARE_ACCOUNTS allocations.
        self.store(SHARES_KEY.to_vec(), &allocations);
    }

    async fn share_mode(&self) -> Result<bool, Error> {
        Ok(self.load(SHARE_MODE_KEY).await?.unwrap_or(false))
    }

    // ---- sibling reads ------------------------------------------------------

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
    /// automation, upgrade tooling, self-serving ops), so node principals are
    /// part of the current actor model.
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
        if self.share_mode().await? {
            let shares = self.shares().await?.ok_or_else(|| {
                Error::Module("account-share mode has no configured registry".into())
            })?;
            let account_id = self.account_principal(ctx, submitter).await?;
            if !shares.contains_key(&account_id) {
                return Err(Error::Module(
                    "submitter account holds no governance shares".into(),
                ));
            }
            let total = Self::total_power(&shares)?;
            return Ok((
                account_id,
                Electorate {
                    voter_kind: VoterKind::Account,
                    rule: Self::threshold_rule(total, action, true),
                    powers: shares,
                },
            ));
        }

        // validator mode (the default): ballots stay NODE-keyed — N
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
                    "submitter holds no validator-set standing (no member node bound to it)".into(),
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
            electorate: electorate
                .powers
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
            voting_rule: electorate.rule,
        }
    }

    async fn shares_view(&self) -> Result<SharesView, Error> {
        let Some(shares) = self.shares().await? else {
            return Ok(SharesView {
                active: false,
                allocations: Vec::new(),
                total: 0,
            });
        };
        Ok(SharesView {
            active: self.share_mode().await?,
            allocations: shares
                .iter()
                .map(|(account_id, shares)| ShareAllocation {
                    account_id: account_id.clone(),
                    shares: *shares,
                })
                .collect(),
            total: shares.values().sum(),
        })
    }

    async fn normalize_share_action(
        &self,
        ctx: &dyn Ctx,
        mut action: GovAction,
    ) -> Result<GovAction, Error> {
        match &mut action {
            GovAction::AdoptShares { allocations } => {
                if self.shares().await?.is_some() {
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
                let current = self.shares().await?.ok_or_else(|| {
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
                if *enabled && self.shares().await?.is_none() {
                    return Err(Error::Module(
                        "configure governance shares before enabling share mode".into(),
                    ));
                }
                if *enabled == self.share_mode().await? {
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
        if proposal_id.len() > MAX_PROPOSAL_ID_BYTES {
            return Err(Error::Module(format!(
                "proposal_id exceeds {MAX_PROPOSAL_ID_BYTES} bytes ({} given)",
                proposal_id.len()
            )));
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
        if let GovAction::UpdateModule {
            name, module_id, ..
        }
        | GovAction::RegisterModule {
            name, module_id, ..
        }
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
        let mut roster = self.roster().await?;
        let position = match roster.binary_search(&proposal_id) {
            Ok(_) => {
                return Err(Error::Module(format!(
                    "proposal already exists: {proposal_id}"
                )));
            }
            Err(position) => position,
        };
        if roster.len() >= MAX_PROPOSALS {
            return Err(Error::Module(format!(
                "proposal cap reached ({MAX_PROPOSALS})"
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
        let proposal = Proposal {
            action,
            proposer,
            created_at: now,
            deadline,
            status: ProposalStatus::Open,
            votes: BTreeMap::new(),
            electorate,
        };
        // both byte gates first: a refusal must stage NOTHING. a NEW proposal
        // is gated at HALF the record cap so accrued ballots (at most one per
        // electorate entry, each no larger than its entry) can never push the
        // settled record past the full cap.
        self.store_bounded(
            prop_key(&proposal_id),
            &ProposalRecord::from(&proposal),
            MAX_PROPOSAL_RECORD_BYTES / 2,
            "proposal",
        )?;
        roster.insert(position, proposal_id);
        self.store_bounded(
            PROPOSAL_ROSTER_KEY.to_vec(),
            &roster,
            MAX_ROSTER_RECORD_BYTES,
            "proposal roster",
        )?;
        Ok(())
    }

    async fn handle_vote(
        &mut self,
        ctx: &mut dyn Ctx,
        proposal_id: String,
        approve: bool,
    ) -> Result<(), Error> {
        let mut proposal = self
            .proposal(&proposal_id)
            .await?
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
                    .node_ballots(ctx, &submitter, &|node| {
                        electorate.powers.contains_key(node)
                    })
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
        self.store_proposal(&proposal_id, &proposal)
    }

    async fn handle_execute(
        &mut self,
        ctx: &mut dyn Ctx,
        proposal_id: String,
    ) -> Result<(), Error> {
        let mut proposal = self
            .proposal(&proposal_id)
            .await?
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
                // adopted from a peer's synced store minted under DIFFERENT
                // genesis wiring — reject it cleanly rather than emit into the
                // void.
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
                    if self.shares().await?.is_some() {
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
                            self.stage_shares(&shares);
                            self.store(SHARE_MODE_KEY.to_vec(), &true);
                        }
                    }
                }
                GovAction::SetShares { account_id, shares } => {
                    let Some(mut after) = self.shares().await? else {
                        proposal.status = ProposalStatus::Rejected;
                        return self.store_proposal(&proposal_id, &proposal);
                    };
                    if *shares == 0 {
                        after.remove(account_id);
                    } else {
                        after.insert(account_id.clone(), *shares);
                    }
                    if after.len() > MAX_SHARE_ACCOUNTS || Self::total_power(&after).is_err() {
                        proposal.status = ProposalStatus::Rejected;
                    } else {
                        self.stage_shares(&after);
                    }
                }
                GovAction::SetShareMode { enabled } => {
                    if *enabled && self.shares().await?.is_none() {
                        proposal.status = ProposalStatus::Rejected;
                    } else {
                        self.store(SHARE_MODE_KEY.to_vec(), enabled);
                    }
                }
                GovAction::Signal { .. } => {}
            }
        } else {
            proposal.status = ProposalStatus::Rejected;
        }
        self.store_proposal(&proposal_id, &proposal)
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
        // (the staged-over-committed read collapses two redemptions in one
        // block to first-wins too). a Client and a Resident invite carry
        // different nonces, but the keyspace is shared so neither token can be
        // replayed as the other's.
        if self.load::<Redemption>(&red_key(&nonce)).await?.is_some() {
            return Err(Error::Module("invite already redeemed".into()));
        }
        self.store(
            red_key(&nonce),
            &Redemption {
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

    /// the store's merkle root over all committed records, verbatim — the
    /// staged overlay is invisible here until `commit_block`.
    fn root(&self) -> StateRoot {
        self.staged.root()
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        self.staged.state_sync_handle()
    }

    /// the network state-sync serve lane: answers the shared qmdb wire requests
    /// (historical proof-carrying op ranges) from committed state. read-only;
    /// the joiner's sync engine merkle-verifies every batch.
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        self.staged.serve_sync(req).await
    }

    async fn resolver_sync_target(&self) -> Result<ResolverSyncTarget, Error> {
        self.staged.sync_target().await
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
                self.handle_redeem(
                    ctx,
                    issuer,
                    nonce,
                    token_sig,
                    joiner,
                    proof,
                    role,
                    expires_unix_secs,
                )
                    .await
            }
        }
    }

    /// read projection — committed plus this block's staged changes (the
    /// staged-over-committed store view).
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            GovQuery::Proposals => {
                // walk the roster by derived key (≤ MAX_PROPOSALS point
                // reads). a rostered id without a record is a store bug —
                // loud, never skipped.
                let mut views = Vec::new();
                for proposal_id in self.roster().await? {
                    let Some(proposal) = self.proposal(&proposal_id).await? else {
                        return Err(Error::Module(format!(
                            "missing proposal record: {proposal_id}"
                        )));
                    };
                    views.push(Self::view_of(&proposal_id, &proposal));
                }
                Ok(encode_reply(&GovReply::Proposals(views)))
            }
            GovQuery::Proposal { proposal_id } => Ok(encode_reply(&GovReply::Proposal(
                self.proposal(&proposal_id)
                    .await?
                    .map(|p| Self::view_of(&proposal_id, &p)),
            ))),
            GovQuery::Redemption { nonce } => {
                let view = self
                    .load::<Redemption>(&red_key(&nonce))
                    .await?
                    .map(|r| RedemptionView {
                        nonce: nonce.clone(),
                        joiner: r.joiner,
                        issuer: r.issuer,
                        height: r.height,
                    });
                Ok(encode_reply(&GovReply::Redemption(view)))
            }
            GovQuery::Shares => Ok(encode_reply(&GovReply::Shares(self.shares_view().await?))),
        }
    }

    /// publish the block's staged writes in ONE store batch. no-op (and no
    /// root movement) if nothing was staged.
    async fn commit_block(&mut self) -> Result<(), Error> {
        self.staged.commit().await
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.abort();
        Ok(())
    }
}

// the wasm-guest port: the store-backed dispatch shell that adapts this module
// to the ducktape:module world. compiled only by the guest-builder's
// synthesized wasm32 cdylib workspace (feature `guest`), never by the native
// build.
#[cfg(feature = "guest")]
mod guest;
