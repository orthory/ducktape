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

use std::collections::BTreeMap;

use governance_interface::{
    GovAction, GovMsg, GovQuery, GovReply, ProposalStatus, ProposalView, decode_msg, decode_query,
    encode_reply,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};
use upgrade_interface::{UpgradeMsg, encode_msg as upgrade_encode_msg};
use valset_interface::{
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

pub struct Governance {
    id: ModuleId,
    /// the id of the valset module this governance instance gates. genesis
    /// wiring — identical on every node.
    valset_id: ModuleId,
    /// the id of the upgrade module a passing `ScheduleUpgrade`/`CancelUpgrade`
    /// authorizes. genesis wiring — identical on every node.
    upgrade_id: ModuleId,
    /// committed proposals — what `root()` commits to.
    proposals: BTreeMap<String, Proposal>,
    /// this block's staged writes (whole-proposal overwrite granularity),
    /// read ahead of committed state, merged at `commit_block`.
    pending: BTreeMap<String, Proposal>,
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
            proposals: BTreeMap::new(),
            pending: BTreeMap::new(),
        }
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

    fn encode_state(proposals: &BTreeMap<String, Proposal>) -> Vec<u8> {
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
        out
    }

    fn root_of(proposals: &BTreeMap<String, Proposal>) -> StateRoot {
        if proposals.is_empty() {
            return StateRoot::ZERO;
        }
        let mut h = Sha256::new();
        h.update(Self::encode_state(proposals));
        StateRoot(h.finalize().into())
    }

    /// canonical bytes of COMMITTED state — the exact preimage of `root()`.
    pub fn snapshot(&self) -> Vec<u8> {
        Self::encode_state(&self.proposals)
    }

    /// verify-then-adopt a peer snapshot: decode into a temporary, recompute
    /// the root, refuse on mismatch — committed state and stage untouched on
    /// any error. success drops the stage (it belonged to the replaced state).
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let decoded = decode_state(bytes)?;
        if Self::root_of(&decoded) != expected {
            return Err(Error::Module("snapshot root mismatch".into()));
        }
        self.proposals = decoded;
        self.pending.clear();
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
        if let GovAction::AddValidator { key } | GovAction::RemoveValidator { key } = &action {
            // shape-check the key here so a proposal that can never execute is
            // rejected at the door, not at tally time.
            if key.len() != 32 {
                return Err(Error::Module(
                    "validator key must be a 32-byte ed25519 public key".into(),
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
                GovAction::Signal { .. } => {}
            }
        } else {
            proposal.status = ProposalStatus::Rejected;
        }
        self.pending.insert(proposal_id, proposal);
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Governance {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// sha256 over the canonical encoding of COMMITTED proposals; `ZERO` when
    /// none exist (the sdk's uninitialized-module sentinel).
    fn root(&self) -> StateRoot {
        Self::root_of(&self.proposals)
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
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (id, p) in std::mem::take(&mut self.pending) {
            self.proposals.insert(id, p);
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
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

fn decode_state(bytes: &[u8]) -> Result<BTreeMap<String, Proposal>, Error> {
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
    if !buf.is_empty() {
        return Err(Error::Module("snapshot carries trailing bytes".into()));
    }
    Ok(proposals)
}
