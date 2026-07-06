//! member-gated governance over the validator set.
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
//! sees it, so `Origin::External(pubkey)` here is AUTHENTICATED: a ballot is
//! attributable to exactly one member key, and no validator can forge another
//! member's vote.
//!
//! ## determinism
//!
//! membership checks are host-routed reads of the valset module's
//! staged-over-committed projection — deterministic across validators because
//! every dispatch sees the same block state. tallies compare yes-ballots
//! against the CURRENT member count at execute time (members = valset
//! projection), so the same Execute op settles identically everywhere.
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
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};
use upgrade::{UpgradeMsg, encode_msg as upgrade_encode_msg};
use valset::{
    ValsetMsg, ValsetQuery, ValsetReply, decode_reply as valset_decode_reply,
    encode_msg as valset_encode_msg, encode_query as valset_encode_query,
};

/// ceiling on `voting_period` (in consensus-time units) — a fat-fingered or
/// hostile period must not park a proposal Open forever past any usable
/// horizon. views advance about once per finalized op, so this is generous.
const MAX_VOTING_PERIOD: u64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Proposal {
    action: GovAction,
    proposer: Vec<u8>,
    created_at: u64,
    deadline: u64,
    status: ProposalStatus,
    votes: BTreeMap<Vec<u8>, bool>,
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
    /// the id of the upgrade module a passing `ScheduleUpgrade`/`CancelUpgrade`
    /// authorizes. genesis wiring — identical on every node.
    upgrade_id: ModuleId,
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
    /// folded into `root()`/`snapshot()` only when non-empty, so every
    /// pre-invite state keeps its historical root bytes.
    redeemed: BTreeMap<Vec<u8>, Redemption>,
    /// this block's staged redemptions, same discipline as `pending`.
    pending_redeemed: BTreeMap<Vec<u8>, Redemption>,
}

impl Governance {
    pub fn new(
        id: impl Into<ModuleId>,
        valset_id: impl Into<ModuleId>,
        upgrade_id: impl Into<ModuleId>,
    ) -> Self {
        Self {
            id: id.into(),
            valset_id: valset_id.into(),
            upgrade_id: upgrade_id.into(),
            invite_binding: None,
            proposals: BTreeMap::new(),
            pending: BTreeMap::new(),
            redeemed: BTreeMap::new(),
            pending_redeemed: BTreeMap::new(),
        }
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

    /// the CURRENT member set: the valset module's staged-over-committed
    /// projection, via the host-routed read lane.
    async fn members(&self, ctx: &dyn Ctx) -> Result<Vec<Vec<u8>>, Error> {
        let reply = ctx
            .query(
                &self.valset_id,
                &valset_encode_query(&ValsetQuery::Validators),
            )
            .await?;
        match valset_decode_reply(&reply).map_err(Error::Module)? {
            ValsetReply::Validators(members) => Ok(members),
            other => Err(Error::Module(format!(
                "valset answered a Validators query with {other:?}"
            ))),
        }
    }

    async fn require_member(&self, ctx: &dyn Ctx, key: &[u8]) -> Result<(), Error> {
        let members = self.members(ctx).await?;
        if members.iter().any(|m| m == key) {
            Ok(())
        } else {
            Err(Error::Module(
                "submitter is not a current validator-set member".into(),
            ))
        }
    }

    /// the CURRENT observer set (valset's staged-over-committed projection) —
    /// the standing a redeemed joiner already holds.
    async fn observers(&self, ctx: &dyn Ctx) -> Result<Vec<Vec<u8>>, Error> {
        let reply = ctx
            .query(&self.valset_id, &valset_encode_query(&ValsetQuery::Observers))
            .await?;
        match valset_decode_reply(&reply).map_err(Error::Module)? {
            ValsetReply::Observers(observers) => Ok(observers),
            other => Err(Error::Module(format!(
                "valset answered an Observers query with {other:?}"
            ))),
        }
    }

    fn view_of(id: &str, p: &Proposal) -> ProposalView {
        ProposalView {
            proposal_id: id.to_string(),
            action: p.action.clone(),
            proposer: p.proposer.clone(),
            created_at: p.created_at,
            deadline: p.deadline,
            status: p.status,
            votes: p.votes.iter().map(|(k, v)| (k.clone(), *v)).collect(),
        }
    }

    // ---- canonical state bytes (root preimage + snapshot format) -----------

    fn encode_state(
        proposals: &BTreeMap<String, Proposal>,
        redeemed: &BTreeMap<Vec<u8>, Redemption>,
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
                GovAction::ScheduleUpgrade {
                    name,
                    activation_height,
                    to_version,
                } => {
                    out.push(3);
                    push_bytes(&mut out, name.as_bytes());
                    out.extend_from_slice(&activation_height.to_le_bytes());
                    out.extend_from_slice(&to_version.to_le_bytes());
                }
                GovAction::CancelUpgrade { name } => {
                    out.push(4);
                    push_bytes(&mut out, name.as_bytes());
                }
                GovAction::AddObserver { key } => {
                    out.push(5);
                    push_bytes(&mut out, key);
                }
                GovAction::RemoveObserver { key } => {
                    out.push(6);
                    push_bytes(&mut out, key);
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
        // the redemption section rides ONLY when non-empty: every pre-invite
        // state keeps its exact historical root and snapshot bytes.
        if !redeemed.is_empty() {
            out.extend_from_slice(&(redeemed.len() as u64).to_le_bytes());
            for (nonce, r) in redeemed {
                push_bytes(&mut out, nonce);
                push_bytes(&mut out, &r.joiner);
                push_bytes(&mut out, &r.issuer);
                out.extend_from_slice(&r.height.to_le_bytes());
            }
        }
        out
    }

    fn root_of(
        proposals: &BTreeMap<String, Proposal>,
        redeemed: &BTreeMap<Vec<u8>, Redemption>,
    ) -> StateRoot {
        if proposals.is_empty() && redeemed.is_empty() {
            return StateRoot::ZERO;
        }
        let mut h = Sha256::new();
        h.update(Self::encode_state(proposals, redeemed));
        StateRoot(h.finalize().into())
    }

    /// canonical bytes of COMMITTED state — the exact preimage of `root()`.
    pub fn snapshot(&self) -> Vec<u8> {
        Self::encode_state(&self.proposals, &self.redeemed)
    }

    /// verify-then-adopt a peer snapshot: decode into a temporary, recompute
    /// the root, refuse on mismatch — committed state and stage untouched on
    /// any error. success drops the stage (it belonged to the replaced state).
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let (proposals, redeemed) = decode_state(bytes)?;
        if Self::root_of(&proposals, &redeemed) != expected {
            return Err(Error::Module("snapshot root mismatch".into()));
        }
        self.proposals = proposals;
        self.redeemed = redeemed;
        self.pending.clear();
        self.pending_redeemed.clear();
        Ok(())
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
        | GovAction::AddObserver { key }
        | GovAction::RemoveObserver { key } = &action
        {
            // shape-check the key here so a proposal that can never execute is
            // rejected at the door, not at tally time.
            if key.len() != 32 {
                return Err(Error::Module(
                    "membership key must be a 32-byte ed25519 public key".into(),
                ));
            }
        }
        // upgrade authorizations must name a non-empty upgrade — an unnamed
        // proposal can never match a real pending, so reject it at the door.
        // monotonicity / min-lead / at-most-one are NOT checked here: those are
        // the upgrade module's sole authority at ingest (do not duplicate).
        if let GovAction::ScheduleUpgrade { name, .. } | GovAction::CancelUpgrade { name } = &action
            && name.is_empty()
        {
            return Err(Error::Module("upgrade name must not be empty".into()));
        }
        if self.get(&proposal_id).is_some() {
            return Err(Error::Module(format!(
                "proposal already exists: {proposal_id}"
            )));
        }
        let proposer = Self::external_origin(ctx)?;
        self.require_member(ctx, &proposer).await?;

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
        let voter = Self::external_origin(ctx)?;
        self.require_member(ctx, &voter).await?;
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
        // re-voting overwrites: the ballot box keys by member, last vote wins.
        proposal.votes.insert(voter, approve);
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

        // tally against the CURRENT member count — membership may have changed
        // since proposing, and the tally must reflect who governs NOW. only
        // CURRENT members' ballots count (a removed member's stale ballot is
        // dead weight, never a vote).
        let members = self.members(ctx).await?;
        let yes = members
            .iter()
            .filter(|m| proposal.votes.get(*m).copied() == Some(true))
            .count();
        let majority = members.len() / 2 + 1;

        let now = ctx.env().consensus_time;
        let decidable_early = yes >= majority;
        if now < proposal.deadline && !decidable_early {
            return Err(Error::Module(format!(
                "not decidable yet: voting open until {} and yes={yes} < majority={majority}",
                proposal.deadline
            )));
        }

        if yes >= majority {
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
                    if members.iter().all(|m| m == key) {
                        proposal.status = ProposalStatus::Rejected;
                    } else {
                        ctx.emit_msg(Msg {
                            target: self.valset_id.clone(),
                            payload: valset_encode_msg(&ValsetMsg::Leave { key: key.clone() }),
                        });
                    }
                }
                // a passing upgrade authorization is PERFORMED the same way: emit
                // the upgrade op as a follow-up. the host drains it in this same
                // block and the upgrade module accepts it because the origin is
                // Module(governance). governance only authorizes; the upgrade
                // module's deterministic gates (monotonicity, min-lead,
                // at-most-one) and the R=n readiness quorum are what ARM it.
                GovAction::ScheduleUpgrade {
                    name,
                    activation_height,
                    to_version,
                } => ctx.emit_msg(Msg {
                    target: self.upgrade_id.clone(),
                    payload: upgrade_encode_msg(&UpgradeMsg::Schedule {
                        name: name.clone(),
                        activation_height: *activation_height,
                        to_version: *to_version,
                    }),
                }),
                GovAction::CancelUpgrade { name } => ctx.emit_msg(Msg {
                    target: self.upgrade_id.clone(),
                    payload: upgrade_encode_msg(&UpgradeMsg::Cancel { name: name.clone() }),
                }),
                // the staged-admission grant/revoke: valset re-gates on the
                // protocol version (defense in depth for direct submits) and
                // owns the validator-overlap rule.
                GovAction::AddObserver { key } => ctx.emit_msg(Msg {
                    target: self.valset_id.clone(),
                    payload: valset_encode_msg(&ValsetMsg::Grant { key: key.clone() }),
                }),
                GovAction::RemoveObserver { key } => ctx.emit_msg(Msg {
                    target: self.valset_id.clone(),
                    payload: valset_encode_msg(&ValsetMsg::Revoke { key: key.clone() }),
                }),
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
    async fn handle_redeem(
        &mut self,
        ctx: &mut dyn Ctx,
        issuer: Vec<u8>,
        nonce: Vec<u8>,
        token_sig: Vec<u8>,
        joiner: Vec<u8>,
        proof: Vec<u8>,
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
        let token = invite::InviteToken {
            issuer: issuer_key,
            nonce: nonce_arr,
            sig,
        };
        if !invite::verify_invite_token(&token, binding) {
            return Err(Error::Module(
                "invite token signature does not verify for this network".into(),
            ));
        }
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
        if members.iter().any(|m| m == &joiner) {
            return Err(Error::Module("joiner is already a validator".into()));
        }
        if self.observers(ctx).await?.iter().any(|o| o == &joiner) {
            return Err(Error::Module("joiner already holds full-node standing".into()));
        }
        // exactly-once: the nonce is the single-use key (pending-over-committed
        // read, so two redemptions in one block settle first-wins too).
        if self.pending_redeemed.contains_key(&nonce) || self.redeemed.contains_key(&nonce) {
            return Err(Error::Module("invite already redeemed".into()));
        }
        self.pending_redeemed.insert(
            nonce,
            Redemption {
                joiner: joiner.clone(),
                issuer,
                height: ctx.env().height,
            },
        );
        ctx.emit_msg(Msg {
            target: self.valset_id.clone(),
            payload: valset_encode_msg(&ValsetMsg::Grant { key: joiner }),
        });
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Governance {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// sha256 over the canonical encoding of COMMITTED proposals (plus the
    /// redemption section when non-empty); `ZERO` when none exist (the sdk's
    /// uninitialized-module sentinel).
    fn root(&self) -> StateRoot {
        Self::root_of(&self.proposals, &self.redeemed)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
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
            } => {
                self.handle_redeem(ctx, issuer, nonce, token_sig, joiner, proof)
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
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (id, p) in std::mem::take(&mut self.pending) {
            self.proposals.insert(id, p);
        }
        for (nonce, r) in std::mem::take(&mut self.pending_redeemed) {
            self.redeemed.insert(nonce, r);
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        self.pending_redeemed.clear();
        Ok(())
    }
}

fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

// ---- strict snapshot decode (untrusted bytes) -------------------------------

fn take_u64(buf: &mut &[u8]) -> Result<u64, Error> {
    let Some((head, rest)) = buf.split_first_chunk::<8>() else {
        return Err(Error::Module("snapshot truncated".into()));
    };
    *buf = rest;
    Ok(u64::from_le_bytes(*head))
}

fn take_u32(buf: &mut &[u8]) -> Result<u32, Error> {
    let Some((head, rest)) = buf.split_first_chunk::<4>() else {
        return Err(Error::Module("snapshot truncated".into()));
    };
    *buf = rest;
    Ok(u32::from_le_bytes(*head))
}

fn take_u8(buf: &mut &[u8]) -> Result<u8, Error> {
    let Some((head, rest)) = buf.split_first() else {
        return Err(Error::Module("snapshot truncated".into()));
    };
    let v = *head;
    *buf = rest;
    Ok(v)
}

fn take_vec(buf: &mut &[u8]) -> Result<Vec<u8>, Error> {
    let len = take_u64(buf)?;
    if len > buf.len() as u64 {
        return Err(Error::Module("snapshot length exceeds buffer".into()));
    }
    let (head, rest) = buf.split_at(len as usize);
    *buf = rest;
    Ok(head.to_vec())
}

fn take_string(buf: &mut &[u8]) -> Result<String, Error> {
    String::from_utf8(take_vec(buf)?).map_err(|_| Error::Module("snapshot: bad utf-8".into()))
}

type DecodedState = (BTreeMap<String, Proposal>, BTreeMap<Vec<u8>, Redemption>);

fn decode_state(bytes: &[u8]) -> Result<DecodedState, Error> {
    let mut buf = bytes;
    let count = take_u64(&mut buf)?;
    // every proposal costs at least its id length prefix — a forged count can
    // never drive allocation past the buffer.
    if count > (buf.len() / 8) as u64 {
        return Err(Error::Module("snapshot count exceeds buffer".into()));
    }
    let mut proposals = BTreeMap::new();
    let mut prev_id: Option<String> = None;
    for _ in 0..count {
        let id = take_string(&mut buf)?;
        // strictly increasing ids: one state has exactly one encoding.
        if prev_id.as_deref().is_some_and(|p| p >= id.as_str()) {
            return Err(Error::Module(
                "snapshot proposal ids must be strictly increasing".into(),
            ));
        }
        let action = match take_u8(&mut buf)? {
            0 => GovAction::AddValidator {
                key: take_vec(&mut buf)?,
            },
            1 => GovAction::RemoveValidator {
                key: take_vec(&mut buf)?,
            },
            2 => GovAction::Signal {
                text: take_string(&mut buf)?,
            },
            3 => GovAction::ScheduleUpgrade {
                name: take_string(&mut buf)?,
                activation_height: take_u64(&mut buf)?,
                to_version: take_u32(&mut buf)?,
            },
            4 => GovAction::CancelUpgrade {
                name: take_string(&mut buf)?,
            },
            5 => GovAction::AddObserver {
                key: take_vec(&mut buf)?,
            },
            6 => GovAction::RemoveObserver {
                key: take_vec(&mut buf)?,
            },
            other => return Err(Error::Module(format!("snapshot: bad action tag {other}"))),
        };
        let proposer = take_vec(&mut buf)?;
        let created_at = take_u64(&mut buf)?;
        let deadline = take_u64(&mut buf)?;
        let status = match take_u8(&mut buf)? {
            0 => ProposalStatus::Open,
            1 => ProposalStatus::Passed,
            2 => ProposalStatus::Rejected,
            other => return Err(Error::Module(format!("snapshot: bad status tag {other}"))),
        };
        let vote_count = take_u64(&mut buf)?;
        if vote_count > (buf.len() / 9) as u64 {
            return Err(Error::Module("snapshot vote count exceeds buffer".into()));
        }
        let mut votes = BTreeMap::new();
        let mut prev_voter: Option<Vec<u8>> = None;
        for _ in 0..vote_count {
            let voter = take_vec(&mut buf)?;
            if prev_voter.as_deref().is_some_and(|p| p >= voter.as_slice()) {
                return Err(Error::Module(
                    "snapshot voters must be strictly increasing".into(),
                ));
            }
            let approve = match take_u8(&mut buf)? {
                0 => false,
                1 => true,
                other => return Err(Error::Module(format!("snapshot: bad ballot {other}"))),
            };
            prev_voter = Some(voter.clone());
            votes.insert(voter, approve);
        }
        prev_id = Some(id.clone());
        proposals.insert(
            id,
            Proposal {
                action,
                proposer,
                created_at,
                deadline,
                status,
                votes,
            },
        );
    }
    // the OPTIONAL redemption section — present iff non-empty (the encoder
    // omits an empty one, so an explicit empty section has no legal encoding).
    let mut redeemed = BTreeMap::new();
    if !buf.is_empty() {
        let rcount = take_u64(&mut buf)?;
        if rcount == 0 {
            return Err(Error::Module(
                "snapshot carries an explicit empty redemption section".into(),
            ));
        }
        if rcount > (buf.len() / 8) as u64 {
            return Err(Error::Module(
                "snapshot redemption count exceeds buffer".into(),
            ));
        }
        let mut prev_nonce: Option<Vec<u8>> = None;
        for _ in 0..rcount {
            let nonce = take_vec(&mut buf)?;
            if prev_nonce.as_deref().is_some_and(|p| p >= nonce.as_slice()) {
                return Err(Error::Module(
                    "snapshot redemption nonces must be strictly increasing".into(),
                ));
            }
            let joiner = take_vec(&mut buf)?;
            let issuer = take_vec(&mut buf)?;
            let height = take_u64(&mut buf)?;
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
    }
    if !buf.is_empty() {
        return Err(Error::Module("snapshot carries trailing bytes".into()));
    }
    Ok((proposals, redeemed))
}
