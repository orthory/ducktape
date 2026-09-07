//! Deployment reconciliation is consensus-module policy. The native bridge
//! sees only opaque submissions and hash-pinned blob staging directives.
use super::*;
use governance::{GovAction, GovMsg, GovQuery, GovReply, ProposalStatus, ProposalView, VotingRule};
use node_work::{Directive, ForgeBlob, Submission};

/// The same voting horizon the operator ceremony uses, in consensus-time units.
const VOTING_PERIOD: u64 = 1_000_000;

fn staged_key(sequence: u64, node: &[u8]) -> String {
    let key = node
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("module_updates/staged/{sequence}/{key}")
}

fn submit<T: serde::Serialize>(target: &str, message: &T) -> Option<Directive> {
    Some(Directive::Submit(Submission {
        target: target.into(),
        payload: sdk::wire::encode(message),
    }))
}

enum Event<'a> {
    Reconcile(u64),
    Wait,
    Stage(&'a ModuleUpdateView),
    Propose {
        sequence: u64,
        action: GovAction,
    },
    Vote {
        proposal: &'a ProposalView,
        node: &'a [u8],
        now: u64,
    },
    Refuse {
        sequence: u64,
        reason: &'static str,
    },
}

fn decide(event: Event<'_>) -> Result<Option<Directive>, Error> {
    match event {
        Event::Reconcile(sequence) => on_reconcile(sequence),
        Event::Wait => on_wait(),
        Event::Stage(view) => on_stage(view),
        Event::Propose { sequence, action } => on_propose(sequence, action),
        Event::Vote {
            proposal,
            node,
            now,
        } => on_vote(proposal, node, now),
        Event::Refuse { sequence, reason } => on_refuse(sequence, reason),
    }
}

fn on_wait() -> Result<Option<Directive>, Error> {
    Ok(None)
}
fn on_reconcile(sequence: u64) -> Result<Option<Directive>, Error> {
    Ok(submit("runs", &RunsMsg::ReconcileModuleUpdate { sequence }))
}
fn on_refuse(sequence: u64, reason: &str) -> Result<Option<Directive>, Error> {
    Ok(submit(
        "runs",
        &RunsMsg::RefuseModuleUpdate {
            sequence,
            reason: reason.into(),
        },
    ))
}
fn on_propose(sequence: u64, action: GovAction) -> Result<Option<Directive>, Error> {
    Ok(submit(
        "governance",
        &GovMsg::Propose {
            proposal_id: module_update_proposal_id(sequence),
            action,
            voting_period: VOTING_PERIOD,
        },
    ))
}
fn on_stage(view: &ModuleUpdateView) -> Result<Option<Directive>, Error> {
    let request = &view.request;
    Ok(Some(Directive::StageBlob {
        blob: ForgeBlob {
            repo: request.source.repo.clone(),
            commit: request.source.commit.clone(),
            path: request.update.artifact.clone(),
            hash: request.update.digest().map_err(Error::Module)?,
        },
        on_ready: Submission {
            target: "runs".into(),
            payload: encode_msg(&RunsMsg::MarkModuleArtifactStaged {
                sequence: request.sequence,
            }),
        },
        on_invalid: Submission {
            target: "runs".into(),
            payload: encode_msg(&RunsMsg::RefuseModuleUpdate {
                sequence: request.sequence,
                reason: "forge artifact is absent or does not match its deployment commitment"
                    .into(),
            }),
        },
    }))
}

fn on_vote(proposal: &ProposalView, node: &[u8], now: u64) -> Result<Option<Directive>, Error> {
    let yes: u64 = proposal
        .electorate
        .iter()
        .filter(|(key, _)| {
            proposal
                .votes
                .iter()
                .any(|(voter, approve)| voter == key && *approve)
        })
        .map(|(_, power)| power)
        .sum();
    let total: u64 = proposal.electorate.iter().map(|(_, power)| power).sum();
    let participating: u64 = proposal
        .electorate
        .iter()
        .filter(|(key, _)| proposal.votes.iter().any(|(voter, _)| voter == key))
        .map(|(_, power)| power)
        .sum();
    let irreversible = match proposal.voting_rule {
        VotingRule::Threshold { required_yes } => yes >= required_yes,
        VotingRule::ParticipatingMajority { quorum } => {
            participating >= quorum && yes > total - yes
        }
    };
    let ready = irreversible || now >= proposal.deadline;
    if ready {
        return Ok(submit(
            "governance",
            &GovMsg::Execute {
                proposal_id: proposal.proposal_id.clone(),
            },
        ));
    }
    let in_electorate = proposal.voter_kind == governance::VoterKind::ValidatorNode
        && proposal.electorate.iter().any(|(key, _)| key == node);
    let voted = proposal.votes.iter().any(|(key, _)| key == node);
    let wait = !in_electorate || voted;
    if wait {
        return Ok(None);
    }
    Ok(submit(
        "governance",
        &GovMsg::Vote {
            proposal_id: proposal.proposal_id.clone(),
            approve: true,
        },
    ))
}

impl RunsModule {
    pub(super) async fn mark_module_artifact_staged(
        &mut self,
        ctx: &mut dyn Ctx,
        sequence: u64,
    ) -> Result<(), Error> {
        let Origin::External(node) = &ctx.env().origin else {
            return Err(Error::Module(
                "artifact residency must be reported by its node key".into(),
            ));
        };
        let members = valset::members(ctx, "valset").await?;
        if !members.contains(node) {
            return Err(Error::Module("artifact reporter is not a validator".into()));
        }
        let Some(view) = self.next_module_update().await? else {
            return Ok(());
        };
        if view.request.sequence != sequence {
            return Ok(());
        }
        self.receipts.stage(
            staged_key(sequence, node),
            sdk::wire::encode(&view.request.update.code_hash),
        )
    }

    pub(super) async fn deployment_work(
        &self,
        ctx: &dyn Ctx,
        node: &[u8],
        height: u64,
        now: u64,
    ) -> Result<Option<Directive>, Error> {
        let Some(view) = self.next_module_update().await? else {
            return Ok(None);
        };
        let members = valset::members(ctx, "valset").await?;
        let member = members.iter().any(|key| key == node);
        if !member {
            return Ok(None);
        }
        let sequence = view.request.sequence;
        let hash = view.request.update.digest().map_err(Error::Module)?;
        let bytes = ctx
            .query(
                "modules",
                &modules::encode_query(&modules::ModulesQuery::ModuleStatus),
            )
            .await?;
        let modules::ModulesReply::ModuleStatus { modules } =
            modules::decode_reply(&bytes).map_err(Error::Module)?
        else {
            return Err(Error::Module("unexpected module registry reply".into()));
        };
        let Some(module) = modules
            .iter()
            .find(|module| module.module_id == view.request.update.module_id)
        else {
            return decide(Event::Refuse {
                sequence,
                reason: "module is not registered",
            });
        };
        let active = module.active_code_hash == hash && module.pending.is_none();
        if active {
            return decide(Event::Reconcile(sequence));
        }
        let action = GovAction::UpdateModule {
            name: module_update_proposal_id(sequence),
            module_id: view.request.update.module_id.clone(),
            activation_lead: view.request.update.after,
            code_hash: hash.to_vec(),
        };
        let bytes = ctx
            .query(
                "governance",
                &governance::encode_query(&GovQuery::Proposal {
                    proposal_id: module_update_proposal_id(sequence),
                }),
            )
            .await?;
        let GovReply::Proposal(proposal) =
            governance::decode_reply(&bytes).map_err(Error::Module)?
        else {
            return Err(Error::Module("unexpected deployment proposal reply".into()));
        };
        let staged = self
            .receipts
            .get(&staged_key(sequence, node))
            .await?
            .is_some();
        let Some(proposal) = proposal else {
            let bytes = ctx
                .query("governance", &governance::encode_query(&GovQuery::Shares))
                .await?;
            let GovReply::Shares(shares) =
                governance::decode_reply(&bytes).map_err(Error::Module)?
            else {
                return Err(Error::Module("unexpected governance shares reply".into()));
            };
            if shares.active {
                return decide(Event::Refuse {
                    sequence,
                    reason: "node-key deployments require validator-ballot governance",
                });
            }
            let busy = module
                .pending
                .as_ref()
                .is_some_and(|pending| !pending.stale_at(height));
            if busy {
                return decide(Event::Wait);
            }
            if !staged {
                return decide(Event::Stage(&view));
            }
            return decide(Event::Propose { sequence, action });
        };
        if proposal.action != action {
            return decide(Event::Reconcile(sequence));
        }
        let phase = match proposal.status {
            ProposalStatus::Open => match staged {
                true => Event::Vote {
                    proposal: &proposal,
                    node,
                    now,
                },
                false => Event::Stage(&view),
            },
            ProposalStatus::Rejected => Event::Reconcile(sequence),
            ProposalStatus::Passed => {
                let waiting = module
                    .pending
                    .as_ref()
                    .is_some_and(|pending| pending.code_hash == hash && !pending.stale_at(height));
                match waiting {
                    true => Event::Wait,
                    false => Event::Reconcile(sequence),
                }
            }
        };
        decide(phase)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proposal(votes: Vec<(Vec<u8>, bool)>) -> ProposalView {
        ProposalView {
            proposal_id: "program-module:0".into(),
            action: GovAction::Signal {
                text: "test".into(),
            },
            proposer: vec![1],
            created_at: 0,
            deadline: 100,
            status: ProposalStatus::Open,
            votes,
            voter_kind: governance::VoterKind::ValidatorNode,
            electorate: vec![(vec![1], 1), (vec![2], 1), (vec![3], 1)],
            voting_rule: VotingRule::Threshold { required_yes: 2 },
        }
    }

    fn ballot(proposal: &ProposalView, key: &[u8], now: u64) -> Option<GovMsg> {
        let Some(Directive::Submit(message)) = on_vote(proposal, key, now).unwrap() else {
            return None;
        };
        assert_eq!(message.target, "governance");
        Some(governance::decode_msg(&message.payload).unwrap())
    }

    #[test]
    fn the_module_owns_voting_and_execution_policy() {
        let first = proposal(vec![(vec![1], true)]);
        assert!(ballot(&first, &[1], 1).is_none());
        assert!(ballot(&first, &[4], 1).is_none());
        assert!(matches!(
            ballot(&first, &[2], 1),
            Some(GovMsg::Vote { approve: true, .. })
        ));
        let declined = proposal(vec![(vec![1], false)]);
        assert!(ballot(&declined, &[1], 1).is_none());
        let majority = proposal(vec![(vec![1], true), (vec![2], true)]);
        assert!(matches!(
            ballot(&majority, &[3], 1),
            Some(GovMsg::Execute { .. })
        ));
        assert!(matches!(
            ballot(&first, &[3], first.deadline),
            Some(GovMsg::Execute { .. })
        ));
    }

    #[test]
    fn a_different_frozen_voting_rule_needs_no_native_executor_change() {
        let mut proposal = proposal(vec![(vec![1], true), (vec![2], true)]);
        proposal.voting_rule = VotingRule::Threshold { required_yes: 3 };
        assert!(matches!(
            ballot(&proposal, &[3], 1),
            Some(GovMsg::Vote { .. })
        ));
        proposal.voting_rule = VotingRule::ParticipatingMajority { quorum: 2 };
        assert!(matches!(
            ballot(&proposal, &[3], 1),
            Some(GovMsg::Execute { .. })
        ));
    }
    #[test]
    fn the_policy_dispatch_is_exhaustive_and_only_delegates() {
        let file = syn::parse_file(include_str!("deployment.rs")).unwrap();
        let function = file
            .items
            .iter()
            .find_map(|item| match item {
                syn::Item::Fn(function) if function.sig.ident == "decide" => Some(function),
                _ => None,
            })
            .unwrap();
        assert_eq!(function.block.stmts.len(), 1);
        let syn::Stmt::Expr(syn::Expr::Match(dispatch), None) = &function.block.stmts[0] else {
            panic!("one match")
        };
        for arm in &dispatch.arms {
            assert!(arm.guard.is_none());
            let variant = match &arm.pat {
                syn::Pat::TupleStruct(pattern) => &pattern.path.segments.last().unwrap().ident,
                syn::Pat::Struct(pattern) => &pattern.path.segments.last().unwrap().ident,
                syn::Pat::Path(pattern) => &pattern.path.segments.last().unwrap().ident,
                _ => panic!("every event must be named"),
            };
            let syn::Expr::Call(call) = &*arm.body else {
                panic!("one delegation")
            };
            let syn::Expr::Path(path) = &*call.func else {
                panic!("named handler")
            };
            assert_eq!(
                path.path.segments.last().unwrap().ident.to_string(),
                format!("on_{}", variant.to_string().to_lowercase())
            );
        }
    }
}
