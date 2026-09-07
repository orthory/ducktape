//! A block-driven node executor for program-requested module deployments.
//! Each validator stages the pinned forge object locally before voting with its own key.
use commonware_cryptography::Signer as _;
use futures::SinkExt as _;
use futures::channel::oneshot;
use governance::{GovAction, GovMsg, GovQuery, GovReply, ProposalStatus, ProposalView, VotingRule};
use runs::{ModuleUpdateView, RunsMsg, RunsQuery, RunsReply};
use sha2::Digest as _;

use crate::{NodeCommand, NodeHandle};

enum Command {
    Governance(GovMsg),
    Reconcile(u64),
    Wait,
}

enum Event<'a> {
    Active(u64),
    Missing {
        id: String,
        action: GovAction,
        voting_period: u64,
    },
    Open {
        proposal: &'a ProposalView,
        me: &'a [u8],
        now: u64,
    },
    Passed {
        sequence: u64,
        pending: Option<&'a modules::ScheduledSwap>,
        height: u64,
    },
    Rejected(u64),
}

fn decide(event: Event<'_>) -> Command {
    match event {
        Event::Active(sequence) => on_active(sequence),
        Event::Missing {
            id,
            action,
            voting_period,
        } => on_missing(id, action, voting_period),
        Event::Open { proposal, me, now } => on_open(proposal, me, now),
        Event::Passed {
            sequence,
            pending,
            height,
        } => on_passed(sequence, pending, height),
        Event::Rejected(sequence) => on_rejected(sequence),
    }
}

fn on_active(sequence: u64) -> Command {
    Command::Reconcile(sequence)
}
fn on_rejected(sequence: u64) -> Command {
    Command::Reconcile(sequence)
}
fn on_passed(sequence: u64, pending: Option<&modules::ScheduledSwap>, height: u64) -> Command {
    match pending.filter(|pending| !pending.stale_at(height)) {
        Some(_) => Command::Wait,
        None => Command::Reconcile(sequence),
    }
}
fn on_missing(id: String, action: GovAction, voting_period: u64) -> Command {
    Command::Governance(GovMsg::Propose {
        proposal_id: id,
        action,
        voting_period,
    })
}

fn on_open(proposal: &ProposalView, me: &[u8], now: u64) -> Command {
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
        return Command::Governance(GovMsg::Execute {
            proposal_id: proposal.proposal_id.clone(),
        });
    }
    let in_electorate = proposal.voter_kind == governance::VoterKind::ValidatorNode
        && proposal.electorate.iter().any(|(key, _)| key == me);
    let voted = proposal.votes.iter().any(|(key, _)| key == me);
    let wait = !in_electorate || voted;
    if wait {
        return Command::Wait;
    }
    Command::Governance(GovMsg::Vote {
        proposal_id: proposal.proposal_id.clone(),
        approve: true,
    })
}

async fn query(handle: &NodeHandle, target: &str, req: Vec<u8>) -> Result<Vec<u8>, String> {
    let (reply, rx) = oneshot::channel();
    handle
        .command_sender()
        .send(NodeCommand::Query {
            target: target.into(),
            req,
            reply,
        })
        .await
        .map_err(|_| "node command lane closed".to_string())?;
    rx.await.map_err(|_| "node dropped query".to_string())?
}

async fn submit(handle: &NodeHandle, target: &str, payload: Vec<u8>) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    let key = handle
        .node_signer
        .as_ref()
        .ok_or("node signer is unavailable")?
        .public_key()
        .as_ref()
        .to_vec();
    handle
        .command_sender()
        .send(NodeCommand::Submit {
            target: target.into(),
            payload,
            origin: key,
            reply,
        })
        .await
        .map_err(|_| "node command lane closed".to_string())?;
    rx.await
        .map_err(|_| "node dropped submission".to_string())??;
    Ok(())
}

async fn execute(handle: &NodeHandle, command: Command) -> Result<(), String> {
    match command {
        Command::Governance(message) => {
            submit(handle, "governance", governance::encode_msg(&message)).await
        }
        Command::Reconcile(sequence) => {
            submit(
                handle,
                "runs",
                runs::encode_msg(&RunsMsg::ReconcileModuleUpdate { sequence }),
            )
            .await
        }
        Command::Wait => Ok(()),
    }
}

fn wanted(view: &ModuleUpdateView) -> Result<GovAction, String> {
    Ok(GovAction::UpdateModule {
        name: runs::module_update_proposal_id(view.request.sequence),
        module_id: view.request.update.module_id.clone(),
        activation_lead: view.request.update.after,
        code_hash: view.request.update.digest()?.to_vec(),
    })
}

enum ArtifactError {
    Unavailable(String),
    Invalid(String),
}

fn read_blob(
    repo: &git2::Repository,
    tree: &git2::Tree<'_>,
    path: &str,
) -> Result<Vec<u8>, ArtifactError> {
    let entry = tree.get_path(std::path::Path::new(path)).map_err(|_| {
        ArtifactError::Invalid("artifact file is absent from the committed tree".into())
    })?;
    let regular_file = matches!(entry.filemode(), 0o100644 | 0o100755);
    if !regular_file {
        return Err(ArtifactError::Invalid(
            "artifact must be a regular git blob".into(),
        ));
    }
    let odb = repo
        .odb()
        .map_err(|error| ArtifactError::Unavailable(error.to_string()))?;
    let (size, kind) = odb
        .read_header(entry.id())
        .map_err(|error| ArtifactError::Unavailable(error.to_string()))?;
    let admissible =
        kind == git2::ObjectType::Blob && size <= crate::module_code::MAX_MODULE_ARTIFACT_BYTES;
    if !admissible {
        return Err(ArtifactError::Invalid(
            "artifact exceeds the module staging byte bound".into(),
        ));
    }
    let blob = repo
        .find_blob(entry.id())
        .map_err(|error| ArtifactError::Unavailable(error.to_string()))?;
    Ok(blob.content().to_vec())
}

fn artifact(
    base: &std::path::Path,
    view: &ModuleUpdateView,
    branch_head: &str,
) -> Result<Vec<u8>, ArtifactError> {
    view.request
        .update
        .validate()
        .map_err(ArtifactError::Invalid)?;
    let name = forge::norm_repo(&view.request.source.repo)
        .map_err(|error| ArtifactError::Invalid(error.to_string()))?;
    let repo = git2::Repository::open(base.join(name))
        .map_err(|error| ArtifactError::Unavailable(error.to_string()))?;
    let oid = git2::Oid::from_str(&view.request.source.commit)
        .map_err(|error| ArtifactError::Invalid(error.to_string()))?;
    let head = git2::Oid::from_str(branch_head)
        .map_err(|error| ArtifactError::Invalid(error.to_string()))?;
    let reachable = head == oid
        || repo
            .graph_descendant_of(head, oid)
            .map_err(|error| ArtifactError::Unavailable(error.to_string()))?;
    if !reachable {
        return Err(ArtifactError::Invalid(
            "artifact commit is outside its committed forge branch".into(),
        ));
    }
    let commit = repo
        .find_commit(oid)
        .map_err(|error| ArtifactError::Unavailable(error.to_string()))?;
    let tree = commit
        .tree()
        .map_err(|error| ArtifactError::Unavailable(error.to_string()))?;
    let component = read_blob(&repo, &tree, &view.request.update.component)?;
    let index = view
        .request
        .update
        .index
        .as_deref()
        .map(|path| read_blob(&repo, &tree, path))
        .transpose()?;
    let bytes = module_artifact::ModuleArtifact { component, index }.encode();
    if bytes.len() > crate::module_code::MAX_MODULE_ARTIFACT_BYTES {
        return Err(ArtifactError::Invalid(
            "deployment exceeds the module staging byte bound".into(),
        ));
    }
    let expected = view
        .request
        .update
        .digest()
        .map_err(ArtifactError::Invalid)?;
    let actual: [u8; 32] = sha2::Sha256::digest(&bytes).into();
    if actual != expected {
        return Err(ArtifactError::Invalid(
            "forge artifact does not match the requested deployment hash".into(),
        ));
    }
    Ok(bytes)
}

async fn stage(handle: &NodeHandle, view: &ModuleUpdateView) -> Result<(), ArtifactError> {
    let bytes = query(
        handle,
        "forge",
        forge::encode_query(&forge::ForgeQuery::ListRefs {
            repo: view.request.source.repo.clone(),
        }),
    )
    .await
    .map_err(ArtifactError::Unavailable)?;
    let forge::ForgeReply::Refs(refs) =
        forge::decode_reply(&bytes).map_err(ArtifactError::Unavailable)?
    else {
        return Err(ArtifactError::Unavailable(
            "unexpected forge refs reply".into(),
        ));
    };
    let Some(head) = refs
        .iter()
        .find(|head| head.name == view.request.source.branch)
    else {
        return Err(ArtifactError::Unavailable(
            "deployment branch is not committed locally".into(),
        ));
    };
    let base = handle
        .forge_repo
        .clone()
        .ok_or_else(|| ArtifactError::Unavailable("forge storage is unavailable".into()))?;
    let view = view.clone();
    let branch_head = head.head.clone();
    let index = handle.index.clone();
    let blobs = handle.blobs.clone();
    tokio::task::spawn_blocking(move || {
        let bytes = artifact(&base, &view, &branch_head)?;
        let index = index
            .as_ref()
            .ok_or_else(|| ArtifactError::Unavailable("module index is unavailable".into()))?;
        crate::compose::validate_deployment(&view.request.update.module_id, &bytes, index)
            .map_err(ArtifactError::Invalid)?;
        blobs.put_chunk(bytes);
        Ok(())
    })
    .await
    .map_err(|error| ArtifactError::Unavailable(error.to_string()))?
}

async fn advance(
    handle: &NodeHandle,
    staged: &mut Option<u64>,
    voting_period: u64,
) -> Result<(), String> {
    let has_runs = handle
        .status
        .current()
        .modules
        .iter()
        .any(|module| module.id == "runs");
    if !has_runs {
        return Ok(());
    }
    let bytes = query(
        handle,
        "runs",
        runs::encode_query(&RunsQuery::NextModuleUpdate),
    )
    .await?;
    let RunsReply::ModuleUpdate(Some(view)) = runs::decode_reply(&bytes)? else {
        return Ok(());
    };
    let me = handle
        .node_signer
        .as_ref()
        .ok_or("node signer is unavailable")?
        .public_key()
        .as_ref()
        .to_vec();
    let bytes = query(
        handle,
        "valset",
        valset::encode_query(&valset::ValsetQuery::Validators),
    )
    .await?;
    let valset::ValsetReply::Validators(members) = valset::decode_reply(&bytes)? else {
        return Err("unexpected validator membership reply".into());
    };
    if !members.contains(&me) {
        return Ok(());
    }
    let bytes = query(
        handle,
        "modules",
        modules::encode_query(&modules::ModulesQuery::ModuleStatus),
    )
    .await?;
    let modules::ModulesReply::ModuleStatus { modules } = modules::decode_reply(&bytes)? else {
        return Err("unexpected module registry reply".into());
    };
    let digest = view.request.update.digest()?;
    let Some(module) = modules
        .iter()
        .find(|module| module.module_id == view.request.update.module_id)
    else {
        return submit(
            handle,
            "runs",
            runs::encode_msg(&RunsMsg::RefuseModuleUpdate {
                sequence: view.request.sequence,
                reason: "module is not registered".into(),
            }),
        )
        .await;
    };
    let active = modules.iter().any(|module| {
        module.module_id == view.request.update.module_id
            && module.active_code_hash == digest
            && module.pending.is_none()
    });
    if active {
        return execute(handle, decide(Event::Active(view.request.sequence))).await;
    }
    let action = wanted(&view)?;
    let id = runs::module_update_proposal_id(view.request.sequence);
    let bytes = query(
        handle,
        "governance",
        governance::encode_query(&GovQuery::Proposal {
            proposal_id: id.clone(),
        }),
    )
    .await?;
    let GovReply::Proposal(proposal) = governance::decode_reply(&bytes)? else {
        return Err("unexpected deployment proposal reply".into());
    };
    let terminal = proposal.as_ref().is_some_and(|proposal| {
        proposal.status == ProposalStatus::Rejected || proposal.action != action
    });
    if terminal {
        return execute(handle, Command::Reconcile(view.request.sequence)).await;
    }
    let passed = proposal
        .as_ref()
        .is_some_and(|proposal| proposal.status == ProposalStatus::Passed);
    if passed {
        return execute(
            handle,
            decide(Event::Passed {
                sequence: view.request.sequence,
                pending: module
                    .pending
                    .as_ref()
                    .filter(|pending| pending.code_hash == digest),
                height: handle.status.current().height,
            }),
        )
        .await;
    }
    if proposal.is_none() {
        let bytes = query(
            handle,
            "governance",
            governance::encode_query(&GovQuery::Shares),
        )
        .await?;
        let GovReply::Shares(shares) = governance::decode_reply(&bytes)? else {
            return Err("unexpected governance shares reply".into());
        };
        if shares.active {
            return submit(
                handle,
                "runs",
                runs::encode_msg(&RunsMsg::RefuseModuleUpdate {
                    sequence: view.request.sequence,
                    reason: "node-key deployments require validator-ballot governance".into(),
                }),
            )
            .await;
        }
        let busy = module
            .pending
            .as_ref()
            .is_some_and(|pending| !pending.stale_at(handle.status.current().height));
        if busy {
            return Ok(());
        }
    }
    if *staged != Some(view.request.sequence) {
        match stage(handle, &view).await {
            Ok(()) => {
                *staged = Some(view.request.sequence);
                tracing::info!(target: "ducktape::modules", sequence = view.request.sequence,
                    module = %view.request.update.module_id, digest = %view.request.update.code_hash,
                    "program deployment staged from forge");
            }
            Err(ArtifactError::Unavailable(reason)) => return Err(reason),
            Err(ArtifactError::Invalid(reason)) => {
                return submit(
                    handle,
                    "runs",
                    runs::encode_msg(&RunsMsg::RefuseModuleUpdate {
                        sequence: view.request.sequence,
                        reason,
                    }),
                )
                .await;
            }
        }
    }
    let Some(proposal) = proposal else {
        return execute(
            handle,
            decide(Event::Missing {
                id,
                action,
                voting_period,
            }),
        )
        .await;
    };
    let phase = match proposal.status {
        ProposalStatus::Open => Event::Open {
            proposal: &proposal,
            me: &me,
            now: handle.status.current().consensus_time,
        },
        ProposalStatus::Passed => Event::Passed {
            sequence: view.request.sequence,
            pending: module
                .pending
                .as_ref()
                .filter(|pending| pending.code_hash == digest),
            height: handle.status.current().height,
        },
        ProposalStatus::Rejected => Event::Rejected(view.request.sequence),
    };
    execute(handle, decide(phase)).await
}

/// Run beside the HTTP surface, outside the consensus drain. Block wakes are hints;
/// the durable queue and ballots make catch-up and restart use the same path.
pub fn spawn(handle: NodeHandle, voting_period: u64) {
    let mut blocks = handle.stream_hub().subscribe_blocks();
    tokio::spawn(async move {
        let mut staged = None;
        let mut failures = 0u64;
        loop {
            match advance(&handle, &mut staged, voting_period).await {
                Ok(()) => failures = 0,
                Err(error) => {
                    failures = failures.saturating_add(1);
                    if failures.is_power_of_two() {
                        tracing::warn!(target: "ducktape::modules", reason = "program_deployment_retry",
                            attempts = failures, error = %error.lines().next().unwrap_or_default(),
                            "program deployment will retry at a committed block");
                    }
                }
            }
            match blocks.recv().await {
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            }
        }
    });
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

    #[test]
    fn validators_vote_once_and_execute_only_a_decided_proposal() {
        let first = proposal(vec![(vec![1], true)]);
        assert!(matches!(on_open(&first, &[1], 1), Command::Wait));
        assert!(matches!(
            on_open(&first, &[2], 1),
            Command::Governance(GovMsg::Vote { approve: true, .. })
        ));
        assert!(matches!(on_open(&first, &[4], 1), Command::Wait));
        let declined = proposal(vec![(vec![1], false)]);
        assert!(matches!(on_open(&declined, &[1], 1), Command::Wait));
        let majority = proposal(vec![(vec![1], true), (vec![2], true)]);
        assert!(matches!(
            on_open(&majority, &[3], 1),
            Command::Governance(GovMsg::Execute { .. })
        ));
        assert!(matches!(
            on_open(&first, &[3], first.deadline),
            Command::Governance(GovMsg::Execute { .. })
        ));
    }

    #[test]
    fn an_expired_swap_reconciles_instead_of_holding_the_queue() {
        let pending = modules::ScheduledSwap {
            name: "program-module:0".into(),
            activation_height: 50,
            code_hash: vec![1; 32],
            readiness: vec![],
            ready_at: None,
        };
        assert!(matches!(on_passed(0, Some(&pending), 49), Command::Wait));
        assert!(matches!(
            on_passed(0, Some(&pending), 50),
            Command::Reconcile(0)
        ));
        let armed = modules::ScheduledSwap {
            ready_at: Some(49),
            ..pending
        };
        assert!(matches!(on_passed(0, Some(&armed), 50), Command::Wait));
        assert!(matches!(on_passed(0, None, 50), Command::Reconcile(0)));
    }

    fn commit(repo: &git2::Repository, data: &[u8], parent: Option<git2::Oid>) -> git2::Oid {
        let blob = repo.blob(data).unwrap();
        let mut builder = repo.treebuilder(None).unwrap();
        builder
            .insert("hello.component.wasm", blob, 0o100644)
            .unwrap();
        let tree = repo.find_tree(builder.write().unwrap()).unwrap();
        let signature = git2::Signature::now("Test", "test@example.invalid").unwrap();
        let parent = parent.map(|oid| repo.find_commit(oid).unwrap());
        let parents = parent.iter().collect::<Vec<_>>();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "artifact",
            &tree,
            &parents,
        )
        .unwrap()
    }

    fn request(commit: git2::Oid, component: &[u8]) -> ModuleUpdateView {
        ModuleUpdateView {
            request: runs::ModuleUpdateRequest {
                sequence: 0,
                request_id: "run/0".into(),
                account: 2,
                run_id: "run".into(),
                source: runs::ModuleUpdateSource {
                    repo: "demo".into(),
                    branch: "main".into(),
                    commit: commit.to_string(),
                },
                update: runs::ModuleUpdateSpec {
                    module_id: "hello".into(),
                    component: "hello.component.wasm".into(),
                    index: None,
                    code_hash: crate::hex_bytes(
                        &module_artifact::ModuleArtifact::component(component.to_vec()).hash(),
                    ),
                    after: governance::MIN_ACTIVATION_LEAD,
                },
            },
            status: runs::ModuleUpdateStatus::Requested,
        }
    }

    #[test]
    fn branch_advancement_cannot_replace_the_committed_artifact() {
        let root = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(root.path().join("demo")).unwrap();
        let original = commit(&repo, b"first component", None);
        let advanced = commit(&repo, b"different component", Some(original));
        let view = request(original, b"first component");
        let bytes = artifact(root.path(), &view, &advanced.to_string())
            .unwrap_or_else(|_| panic!("pinned artifact"));
        assert_eq!(
            module_artifact::ModuleArtifact::decode(&bytes)
                .unwrap()
                .component,
            b"first component"
        );
        let wrong = request(original, b"different component");
        assert!(matches!(
            artifact(root.path(), &wrong, &advanced.to_string()),
            Err(ArtifactError::Invalid(_))
        ));
    }

    #[test]
    fn an_artifact_cannot_escape_the_repository_or_name_an_uncommitted_revision() {
        let root = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(root.path().join("demo")).unwrap();
        let original = commit(&repo, b"component", None);
        let descendant = commit(&repo, b"future", Some(original));
        let future = request(descendant, b"future");
        assert!(matches!(
            artifact(root.path(), &future, &original.to_string()),
            Err(ArtifactError::Invalid(_))
        ));
        let mut traversal = request(original, b"component");
        traversal.request.update.component = "../secret".into();
        assert!(matches!(
            artifact(root.path(), &traversal, &original.to_string()),
            Err(ArtifactError::Invalid(_))
        ));
        let mut bad_repo = request(original, b"component");
        bad_repo.request.source.repo = "../secret".into();
        assert!(matches!(
            artifact(root.path(), &bad_repo, &original.to_string()),
            Err(ArtifactError::Invalid(_))
        ));
    }

    #[test]
    fn the_event_dispatch_is_exhaustive_and_only_delegates() {
        let source = syn::parse_file(include_str!("module_updates.rs")).unwrap();
        let function = source
            .items
            .into_iter()
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
