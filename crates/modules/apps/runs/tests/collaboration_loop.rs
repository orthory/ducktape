//! The program chooses model work; the worker returns data; program calls
//! apply the validated result. Job lifecycle and source writes are separate receipts.
mod support;
use futures::executor::block_on;
use sdk::Msg;
use support::*;

fn response(task: Option<&str>) -> Vec<u8> {
    let response = runs::AgentResponse {
        reply_blocks: Vec::new(),
        actions: task.into_iter().map(|id| runs::AgentAction::CreateTask {
            task_id: id.into(), title: "From model result".into(),
        }).collect(),
        commit_message: None,
    };
    sdk::wire::encode(&serde_json::json!({
        "ducktape_runner_result": 1,
        "response_text": String::from_utf8(runs::encode_response(&response)).unwrap(),
        "workspace_receipt": {
            "source_prefix": "/shared/agent-workspaces/builder", "output_snapshot": null,
            "commit_height": null, "rebased": false, "no_changes": true,
        },
    }))
}

async fn saga(network: &Network, run: &runs::PendingRun) -> String {
    let bytes = network.host.query("dispatch", &dispatch::encode_query(&dispatch::DispatchQuery::Dispatch {
        receiver: "runs".into(), dispatch_id: run.dispatch_id.clone(),
    })).await.unwrap();
    let dispatch::DispatchReply::Dispatch(Some(view)) = dispatch::decode_reply(&bytes).unwrap() else { panic!("dispatch"); };
    let dispatch::DispatchStatus::AwaitingResult { saga_id } = view.status else { panic!("awaiting result"); };
    saga_id
}

async fn settle(network: &mut Network, run: &runs::PendingRun, outcome: Result<Vec<u8>, String>, accepted: bool) {
    let saga_id = saga(network, run).await;
    if !accepted {
        network.submit(provider(), msg("saga", &saga::SagaMsg::Accept { saga_id: saga_id.clone(), attempt: 0 })).await;
    }
    network.submit(provider(), msg("saga", &saga::SagaMsg::OracleResult { saga_id, attempt: 0, outcome, usage: None })).await;
    network.drain().await;
}

async fn job(network: &Network, id: &str) -> tasks::Job {
    let bytes = network.host.query("tasks", &tasks::encode_job_query(&tasks::JobsQuery::Get { job_id: id.into() })).await.unwrap();
    let tasks::JobsReply::Job(Some(job)) = tasks::decode_job_reply(&bytes).unwrap() else { panic!("job"); };
    job
}

async fn job_submit(network: &mut Network, id: &str, model: &str) {
    network.submit(member(), Msg { target: "tasks".into(), payload: tasks::encode_job_msg(&tasks::JobsMsg::Submit {
        job_id: id.into(), kind: format!("agent/{model}"), spec: "Do the requested work".into(),
    }) }).await;
}

async fn job_network() -> Network {
    let mut network = Network::new().await;
    let run_id = network.provision().await;
    let run = network.runs().await.into_iter().find(|run| run.run_id == run_id).unwrap();
    settle(&mut network, &run, Ok(response(None)), true).await;
    network.submit(member(), msg("runs", &runs::RunsMsg::EnableJobWorker { enabled: true })).await;
    network
}

#[test]
fn job_submission_commits_before_its_program_requests_model_work() {
    block_on(async {
        let mut network = job_network().await;
        job_submit(&mut network, "one", "builder").await;
        assert_eq!(job(&network, "one").await.status, tasks::JobStatus::Pending);
        assert!(network.runs().await.is_empty());
        network.drain().await;
        let processing = job(&network, "one").await;
        assert_eq!(processing.status, tasks::JobStatus::Processing);
        assert_eq!(processing.claim.unwrap().worker, tasks::Party::Module("runs".into()));
        let run = network.runs().await.pop().unwrap();
        assert_eq!(run.job_id.as_deref(), Some("one"));
        settle(&mut network, &run, Ok(response(Some("from-job"))), false).await;
        assert_eq!(job(&network, "one").await.status, tasks::JobStatus::Done);
        assert_eq!(network.task("from-job").await.unwrap().owner, tasks::Party::Account(2));
        assert!(network.runs().await.is_empty());
    });
}

#[test]
fn unknown_model_jobs_stay_pending_and_worker_failures_finalize_with_detail() {
    block_on(async {
        let mut network = job_network().await;
        job_submit(&mut network, "unknown", "missing").await;
        network.drain().await;
        assert_eq!(job(&network, "unknown").await.status, tasks::JobStatus::Pending);
        job_submit(&mut network, "failed", "builder").await;
        network.drain().await;
        let run = network.runs().await.pop().unwrap();
        settle(&mut network, &run, Err("model provider refused".into()), false).await;
        let failed = job(&network, "failed").await;
        assert_eq!(failed.status, tasks::JobStatus::Done);
        let result = failed.result.unwrap();
        assert!(!result.ok);
        assert!(result.payload.contains("model provider refused"));
    });
}

#[test]
fn reusing_a_pruned_job_id_creates_a_fresh_program_invocation_and_run() {
    block_on(async {
        let mut network = job_network().await;
        job_submit(&mut network, "episode", "builder").await;
        network.drain().await;
        let first = network.runs().await.pop().unwrap();
        settle(&mut network, &first, Ok(response(None)), false).await;
        network.submit(member(), Msg { target: "tasks".into(), payload: tasks::encode_job_msg(&tasks::JobsMsg::Prune { job_id: "episode".into() }) }).await;
        job_submit(&mut network, "episode", "builder").await;
        network.drain().await;
        let second = network.runs().await.pop().unwrap();
        assert_ne!(first.run_id, second.run_id);
        assert!(second.job_claim_height > first.job_claim_height);
        settle(&mut network, &second, Ok(response(Some("second-episode"))), false).await;
        assert_eq!(network.task("second-episode").await.unwrap().owner, tasks::Party::Account(2));
    });
}
