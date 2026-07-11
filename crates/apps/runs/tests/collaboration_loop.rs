//! the collaboration loop end-to-end under a real host (design §3), on the
//! tagging + dispatch planes: a human posts with a mention → chat reports the
//! post to the TAGGING plane → the plane's engagement event, the pending
//! entry, the dispatch, and its saga trigger all commit IN THE SAME BLOCK as
//! the post (P2) → the mock worker answers the WorkerRequest effect as an
//! ordinary oracle op → the saga's terminal transition and the dispatch
//! module's contract-checked mailbox write commit in that op's block — and
//! NOTHING reaches the runs module yet (the never-pop-stack rule) → the NEXT block
//! injects the delivery, and the runs module's validated reply (authored as the
//! AGENT) plus the task action commit in that delivery block (pruning the
//! pending entry — the dispatch module keeps the history) → a second
//! composition replaying the identical op sequence lands on the
//! byte-identical app-hash — the oracle-as-op laundering that makes a
//! non-deterministic LLM consensus-safe (N2).
//!
//! the JOBS lane rides the same plane: a submitted `agent/{id}` job is
//! claimed and dispatched in the submit cascade, and the delivery block
//! finalizes the board item with the validated response.

use agent::AgentModule;
use agent::{
    ACTION_CHAT_POST, ACTION_TASKS_CREATE, AgentAction, AgentMsg, AgentQuery, AgentReply,
    AgentResponse, AgentStatus, ReplyBlock, decode_reply, encode_msg, encode_query,
    encode_response,
};
use runs::{RunsModule, dispatch_id_for, job_run_id_for, reply_message_id, run_id_for};
use runs::{
    PendingRun, RunsMsg, RunsQuery, RunsReply, TurnPolicy, decode_reply as runs_decode_reply,
    encode_msg as runs_encode_msg, encode_query as runs_encode_query,
};
use chat::Chat;
use chat::{
    AuthorRef, Block, ChatMsg, ChatQuery, ChatReply, Mark, PostPolicy, Span,
    decode_reply as chat_decode_reply, encode_msg as chat_encode_msg,
    encode_query as chat_encode_query,
};
use commonware_runtime::{Runner as _, deterministic};
use dispatch::DispatchModule;
use dispatch::{
    DispatchQuery, DispatchReply, DispatchStatus, decode_work_spec,
    encode_query as dispatch_encode_query,
};
use host::{BlockContext, Host};
use jobs::Jobs;
use jobs::{
    Job, JobStatus, JobsMsg, JobsQuery, JobsReply, decode_reply as jobs_decode_reply,
    encode_msg as jobs_encode_msg, encode_query as jobs_encode_query,
};
use saga::SagaModule;
use saga::{
    SagaMsg, SagaQuery, SagaReply, SagaStatus, decode_reply as saga_decode_reply,
    decode_worker_request, encode_msg as saga_encode_msg, encode_query as saga_encode_query,
};
use sdk::{Effect, Msg, Origin, StateRoot};
use tagging::TaggingModule;
use tasks::Tasks;
use tasks::{
    TaskQuery, TaskReply, decode_reply as tasks_decode_reply, encode_query as tasks_encode_query,
};

/// the minimal host-assembled runner-result wrapper the oracle now ALWAYS
/// delivers around the model's raw text (marker-less flat results are a
/// flag-day reject).
fn wrap_runner(prose: Vec<u8>) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "ducktape_runner_result": 1,
        "response_text": String::from_utf8(prose).expect("utf-8 prose"),
        "workspace_receipt": {
            "source_prefix": "/shared/agent-workspaces/bot",
            "output_snapshot": null,
            "commit_height": null,
            "rebased": false,
            "no_changes": true
        }
    }))
    .expect("wrapper serializes")
}

/// the model's RAW text (here: a strict AgentResponse JSON — the runs
/// module's in-consensus normalization accepts it as-is).
fn canned_response(run_id: &str) -> Vec<u8> {
    encode_response(&AgentResponse {
        reply_blocks: vec![ReplyBlock {
            kind: "paragraph".into(),
            text: format!("quack: handling {run_id}"),
            lang: None,
        }],
        actions: vec![AgentAction::CreateTask {
            task_id: "task-1".into(),
            title: "follow up on the mention".into(),
        }],
    })
}

/// an actions-only response — what a JOB run is expected to answer with.
fn job_response(task_id: &str, title: &str) -> Vec<u8> {
    encode_response(&AgentResponse {
        reply_blocks: Vec::new(),
        actions: vec![AgentAction::CreateTask {
            task_id: task_id.into(),
            title: title.into(),
        }],
    })
}

/// stand in for the off-consensus worker: decode the effect's WorkerRequest,
/// kind-gate the dispatch WorkSpec, and answer with `raw` through the normal
/// oracle-op submit path, echoing the request's idempotency key.
fn oracle_op_for(effect: &Effect, raw: Vec<u8>) -> Msg {
    let request = decode_worker_request(&effect.0).expect("a WorkerRequest effect");
    let work = decode_work_spec(&request.spec).expect("a dispatch WorkSpec");
    assert_eq!(work.capability, "mock-llm-1");
    Msg {
        target: "saga".into(),
        payload: saga_encode_msg(&SagaMsg::OracleResult {
            saga_id: request.saga_id,
            attempt: request.attempt,
            outcome: Ok(raw),
            usage: None,
        }),
    }
}

fn alice() -> Origin {
    Origin::External(b"alice".to_vec())
}

fn at(height: u64, origin: Origin) -> BlockContext {
    BlockContext {
        protocol_version: 0,
        height,
        consensus_time: height,
        origin,
    }
}

fn as_user(byte: u8, height: u64) -> BlockContext {
    at(height, Origin::External(vec![byte; 32]))
}

fn quackbot_ref() -> AuthorRef {
    AuthorRef::Agent {
        module: "runs".into(),
        agent_id: "quackbot".into(),
    }
}

/// a benign op to advance one block — what triggers the host's committed
/// delivery injection.
fn noop_block(n: u64) -> Msg {
    Msg {
        target: "chat".into(),
        payload: chat_encode_msg(&ChatMsg::CreateChannel {
            channel_id: format!("noop-{n}"),
            name: "Noop".into(),
            post_policy: PostPolicy::Open,
        }),
    }
}

fn register_quackbot(actions: Vec<String>) -> Msg {
    Msg {
        target: "agent".into(),
        payload: encode_msg(&AgentMsg::RegisterAgent {
            agent_id: "quackbot".into(),
            display_name: "Quackbot".into(),
            capability: "mock-llm-1".into(),
            prompt_hash: vec![7u8; 32],
            allowed_actions: actions,
            recipe_hash: None,
            caps: None,
            skills: None,
        }),
    }
}

/// the four setup + post ops, in consensus order. block heights ride along so
/// the replay composition sees the identical `BlockContext`s.
fn scripted_ops() -> Vec<(u64, Origin, Msg)> {
    vec![
        (
            1,
            alice(),
            Msg {
                target: "chat".into(),
                payload: chat_encode_msg(&ChatMsg::CreateChannel {
                    channel_id: "general".into(),
                    name: "General".into(),
                    post_policy: PostPolicy::Open,
                }),
            },
        ),
        (
            2,
            alice(),
            register_quackbot(vec![ACTION_CHAT_POST.into(), ACTION_TASKS_CREATE.into()]),
        ),
        (
            3,
            alice(),
            Msg {
                target: "runs".into(),
                payload: runs_encode_msg(&RunsMsg::WatchChannel {
                    channel_id: "general".into(),
                    policy: TurnPolicy::Mention,
                }),
            },
        ),
        (
            4,
            alice(),
            Msg {
                target: "chat".into(),
                payload: chat_encode_msg(&ChatMsg::PostMessage {
                    channel_id: "general".into(),
                    message_id: "m1".into(),
                    blocks: vec![Block::Paragraph(vec![
                        Span::plain("hey "),
                        Span {
                            text: "@quackbot".into(),
                            marks: vec![Mark::Mention(quackbot_ref())],
                        },
                        Span::plain(" can you pick this up?"),
                    ])],
                    thread: None,
                    as_agent: None,
                }),
            },
        ),
    ]
}

async fn genesis(context: deterministic::Context) -> Host {
    let chat = Chat::init(context, "chat").await.with_tagging("tagging");
    Host::genesis(vec![
        Box::new(chat),
        Box::new(TaggingModule::new("tagging")),
        Box::new(SagaModule::new("saga")),
        Box::new(DispatchModule::new("dispatch", "saga")),
        Box::new(AgentModule::new("agent", "saga", Some("runs".into()))),
        Box::new(RunsModule::new(
            "runs",
            "chat",
            "saga",
            "tagging",
            "dispatch",
            "agent",
            Some("tasks".into()),
            Some("jobs".into()),
        )),
        Box::new(Tasks::new("tasks")),
        Box::new(Jobs::new("jobs")),
    ])
    .expect("genesis")
}

async fn pending_runs(host: &Host) -> Vec<PendingRun> {
    let reply = host
        .query("runs", &runs_encode_query(&RunsQuery::PendingRuns))
        .await
        .unwrap();
    match runs_decode_reply(&reply).unwrap() {
        RunsReply::PendingRuns(runs) => runs,
        other => panic!("unexpected reply: {other:?}"),
    }
}

async fn pending_run(host: &Host, run_id: &str) -> Option<PendingRun> {
    pending_runs(host)
        .await
        .into_iter()
        .find(|p| p.run_id == run_id)
}

async fn saga_status(host: &Host, saga_id: &str) -> Option<SagaStatus> {
    let reply = host
        .query(
            "saga",
            &saga_encode_query(&SagaQuery::Get {
                saga_id: saga_id.into(),
            }),
        )
        .await
        .unwrap();
    match saga_decode_reply(&reply).unwrap() {
        SagaReply::Saga(view) => view.map(|v| v.status),
        other => panic!("expected Saga reply, got {other:?}"),
    }
}

/// a run's dispatch as the dispatch module sees it — the lifecycle ledger,
/// including the INTERNAL saga id the failure tests feed oracle results to.
async fn agent_dispatch(host: &Host, run_id: &str) -> dispatch::DispatchView {
    let reply = host
        .query(
            "dispatch",
            &dispatch_encode_query(&DispatchQuery::Dispatch {
                receiver: "runs".into(),
                dispatch_id: dispatch_id_for(run_id),
            }),
        )
        .await
        .unwrap();
    match dispatch::decode_reply(&reply).unwrap() {
        DispatchReply::Dispatch(Some(view)) => view,
        other => panic!("expected the run's dispatch, got {other:?}"),
    }
}

/// the saga id a still-awaiting run's work rides on.
async fn dispatch_saga_id(host: &Host, run_id: &str) -> String {
    match agent_dispatch(host, run_id).await.status {
        DispatchStatus::AwaitingResult { saga_id } => saga_id,
        other => panic!("the run's dispatch must await its saga, got {other:?}"),
    }
}

async fn chat_message(host: &Host, message_id: &str) -> Option<chat::MessageView> {
    let reply = host
        .query(
            "chat",
            &chat_encode_query(&ChatQuery::Message {
                message_id: message_id.into(),
            }),
        )
        .await
        .unwrap();
    match chat_decode_reply(&reply).unwrap() {
        ChatReply::Message(view) => view,
        other => panic!("unexpected reply: {other:?}"),
    }
}

async fn task_ids(host: &Host) -> Vec<String> {
    let reply = host
        .query("tasks", &tasks_encode_query(&TaskQuery::List))
        .await
        .unwrap();
    match tasks_decode_reply(&reply).unwrap() {
        TaskReply::Tasks(tasks) => tasks.into_iter().map(|t| t.id).collect(),
    }
}

async fn job_view(host: &Host, job_id: &str) -> Option<Job> {
    let reply = host
        .query(
            "jobs",
            &jobs_encode_query(&JobsQuery::Get {
                job_id: job_id.into(),
            }),
        )
        .await
        .unwrap();
    match jobs_decode_reply(&reply).unwrap() {
        JobsReply::Job(job) => job,
        other => panic!("unexpected reply: {other:?}"),
    }
}

fn jobs_msg(payload: JobsMsg) -> Msg {
    Msg {
        target: "jobs".into(),
        payload: jobs_encode_msg(&payload),
    }
}

fn enable_job_worker() -> Msg {
    Msg {
        target: "runs".into(),
        payload: runs_encode_msg(&RunsMsg::EnableJobWorker { enabled: true }),
    }
}

fn submit_job(job_id: &str, agent_id: &str, spec: &str) -> Msg {
    jobs_msg(JobsMsg::Submit {
        job_id: job_id.into(),
        kind: format!("agent/{agent_id}"),
        spec: spec.into(),
    })
}

fn prune_job(job_id: &str) -> Msg {
    jobs_msg(JobsMsg::Prune {
        job_id: job_id.into(),
    })
}

fn reclaim_job(job_id: &str) -> Msg {
    jobs_msg(JobsMsg::Reclaim {
        job_id: job_id.into(),
    })
}

fn claim_job(job_id: &str) -> Msg {
    jobs_msg(JobsMsg::Claim {
        job_id: job_id.into(),
        lease_views: 1000,
    })
}

fn register_duck() -> Msg {
    Msg {
        target: "agent".into(),
        payload: encode_msg(&AgentMsg::RegisterAgent {
            agent_id: "duck".into(),
            display_name: "Duck".into(),
            capability: "mock-llm-1".into(),
            prompt_hash: vec![9u8; 32],
            allowed_actions: vec![ACTION_TASKS_CREATE.into()],
            recipe_hash: None,
            caps: None,
            skills: None,
        }),
    }
}

/// feed an oracle result to a run's dispatch saga — the off-consensus seam.
async fn oracle_result(host: &mut Host, run_id: &str, height: u64, outcome: Result<Vec<u8>, String>) {
    let saga_id = dispatch_saga_id(host, run_id).await;
    host.submit_at(
        at(height, Origin::External(b"oracle".to_vec())),
        Msg {
            target: "saga".into(),
            payload: saga_encode_msg(&SagaMsg::OracleResult {
                saga_id,
                attempt: 0,
                outcome,
                usage: None,
            }),
        },
    )
    .await
    .expect("oracle block");
}

#[test]
fn a_mention_flows_through_tagging_and_dispatch_and_lands_reply_and_task_next_block() {
    let run_id = run_id_for("general", 1, "quackbot");
    let replay_run_id = run_id.clone();

    // ---- instance one: the live flow, block by block ------------------------
    let (settled_hash, oracle_op) = deterministic::Runner::default().start(|context| async move {
        let mut host = genesis(context).await;

        // blocks 1..3: channel, registry (+ the agent's dispatch recipe),
        // watch (+ the plane subscription) — each pair one atomic block (P2).
        let ops = scripted_ops();
        for (height, origin, op) in &ops[..3] {
            host.submit_at(at(*height, origin.clone()), op.clone())
                .await
                .expect("setup block");
        }

        // block 4: the user post. THE SAME BLOCK must carry the message, the
        // tag report, the plane's engagement delivery, the pending entry, the
        // dispatch, and its saga trigger (P2) — and emit exactly one
        // WorkerRequest effect for the off-consensus seam.
        let (height, origin, post) = &ops[3];
        let outcome = host
            .submit_at(at(*height, origin.clone()), post.clone())
            .await
            .expect("post block");
        let entry = pending_run(&host, &run_id).await.expect("entry staged");
        assert_eq!(
            entry.dispatch_id,
            dispatch_id_for(&run_id),
            "the entry correlates the dispatch staged in the SAME block as the post"
        );
        let saga_id = dispatch_saga_id(&host, &run_id).await;
        assert_eq!(
            saga_status(&host, &saga_id).await,
            Some(SagaStatus::Pending),
            "the dispatch's saga is pending in the SAME block too"
        );
        assert_eq!(outcome.effects.len(), 1, "one WorkerRequest effect");

        // the effect's spec is a kind-gated dispatch WorkSpec whose payload
        // is the ENTIRE composed model input — instructions + transcript.
        let request = decode_worker_request(&outcome.effects[0].0).unwrap();
        let work = decode_work_spec(&request.spec).unwrap();
        assert_eq!(work.dispatch_id, dispatch_id_for(&run_id));
        assert_eq!(work.capability, "mock-llm-1");
        let payload_text = String::from_utf8(work.payload.clone()).unwrap();
        assert!(payload_text.contains("Return ONLY a JSON object"));
        assert!(payload_text.contains("@quackbot"));

        // nothing downstream exists yet: the reply and the task wait on the
        // oracle op.
        assert_eq!(chat_message(&host, &reply_message_id(&run_id)).await, None);
        assert_eq!(task_ids(&host).await, Vec::<String>::new());

        // block 5: the worker's oracle op. the saga settles, the dispatch
        // module judges the Text contract and commits the outcome into its
        // MAILBOX — and the runs module sees NOTHING this block (never pop-stack).
        let oracle_op = oracle_op_for(&outcome.effects[0], wrap_runner(canned_response(&run_id)));
        host.submit_at(
            at(5, Origin::External(b"oracle".to_vec())),
            oracle_op.clone(),
        )
        .await
        .expect("oracle block");
        assert_eq!(saga_status(&host, &saga_id).await, Some(SagaStatus::Done));
        assert_eq!(
            agent_dispatch(&host, &run_id).await.status,
            DispatchStatus::AwaitingDelivery
        );
        assert!(
            pending_run(&host, &run_id).await.is_some(),
            "the entry stays pending until the delivery block"
        );
        assert_eq!(
            chat_message(&host, &reply_message_id(&run_id)).await,
            None,
            "a result never reaches its receiver in the block that agreed on it"
        );

        // block 6: ANY next block injects the delivery. the ResultEvent, the
        // validated reply (authored as the AGENT), and the task action all
        // commit in this one delivery block — and the pending entry prunes.
        host.submit_at(at(6, alice()), noop_block(6))
            .await
            .expect("delivery block");
        assert_eq!(
            pending_run(&host, &run_id).await,
            None,
            "the delivered entry pruned — the dispatch module holds the history"
        );
        assert_eq!(
            agent_dispatch(&host, &run_id).await.status,
            DispatchStatus::Delivered
        );
        let reply = chat_message(&host, &reply_message_id(&run_id))
            .await
            .expect("the agent's reply landed in chat");
        assert_eq!(
            reply.head.author,
            quackbot_ref(),
            "the reply is authored as the AGENT, not the bare module"
        );
        assert_eq!(
            reply.head.blocks,
            vec![Block::paragraph(format!("quack: handling {run_id}"))]
        );
        assert_eq!(task_ids(&host).await, vec!["task-1".to_string()]);

        // loop prevention held PLANE-SIDE: the agent's reply was reported to
        // the tagging plane too, but an entity-authored post fires nothing —
        // no new pending entry, no second dispatch.
        assert_eq!(
            pending_runs(&host).await,
            Vec::<PendingRun>::new(),
            "the reply spawned no new run"
        );

        // the registry commits WHICH model+prompt answered (auditability).
        let record = host
            .query(
                "agent",
                &encode_query(&AgentQuery::Agent {
                    agent_id: "quackbot".into(),
                }),
            )
            .await
            .unwrap();
        let AgentReply::Agent(Some(record)) = decode_reply(&record).unwrap() else {
            panic!("agent record expected");
        };
        assert_eq!(record.status, AgentStatus::Active);
        assert_eq!(record.capability, "mock-llm-1");

        (host.app_hash(), oracle_op)
    });

    // ---- instance two: replay the identical op sequence -------------------
    // a fresh composition applies the same six blocks and must land on the
    // byte-identical app-hash: the LLM's non-determinism was laundered into
    // an ordered op, so replay is pure state-machine (N2).
    let replayed_hash = deterministic::Runner::default().start(|context| async move {
        let mut host = genesis(context).await;
        for (height, origin, op) in scripted_ops() {
            host.submit_at(at(height, origin), op)
                .await
                .expect("replayed block");
        }
        host.submit_at(at(5, Origin::External(b"oracle".to_vec())), oracle_op)
            .await
            .expect("replayed oracle op");
        let outcome = host
            .submit_at(at(6, alice()), noop_block(6))
            .await
            .expect("replayed delivery block");
        assert_eq!(pending_run(&host, &replay_run_id).await, None);
        outcome.app_hash
    });

    assert_eq!(
        settled_hash, replayed_hash,
        "two instances, one op sequence -> byte-identical app-hash"
    );
    assert_ne!(settled_hash, StateRoot::ZERO);
}

#[test]
fn an_agent_job_is_claimed_and_dispatched_in_the_submit_cascade() {
    deterministic::Runner::default().start(|context| async move {
        let mut host = genesis(context).await;
        host.submit_at(as_user(1, 1), enable_job_worker())
            .await
            .expect("enable the runs module as the single jobs worker");
        host.submit_at(as_user(1, 2), register_duck())
            .await
            .expect("register duck");

        let spec = "summarize this work item";
        let outcome = host
            .submit_at(as_user(2, 3), submit_job("job-1", "duck", spec))
            .await
            .expect("submit cascades to agent claim + dispatch");

        let job = job_view(&host, "job-1").await.expect("job exists");
        assert_eq!(job.status, JobStatus::Processing);
        assert_eq!(
            job.claim.as_ref().map(|claim| claim.worker.as_str()),
            Some("runs")
        );

        let run_id = job_run_id_for("job-1", "duck", 3);
        let entry = pending_run(&host, &run_id).await.expect("job entry exists");
        assert_eq!(entry.job_id, Some("job-1".into()));
        assert_eq!(entry.job_claim_height, 3);
        assert_eq!(entry.agent_id, "duck");

        // the submit cascade emitted the dispatch's WorkerRequest effect, and
        // its payload carries the FULL job spec — composed in consensus.
        assert_eq!(outcome.effects.len(), 1, "one WorkerRequest effect");
        let request = decode_worker_request(&outcome.effects[0].0).unwrap();
        let work = decode_work_spec(&request.spec).unwrap();
        assert_eq!(work.dispatch_id, dispatch_id_for(&run_id));
        let payload_text = String::from_utf8(work.payload).unwrap();
        assert!(payload_text.contains(spec), "the job spec rides the payload");
    });
}

#[test]
fn an_agent_job_for_an_unknown_agent_commits_without_a_claim() {
    deterministic::Runner::default().start(|context| async move {
        let mut host = genesis(context).await;
        host.submit_at(as_user(1, 1), enable_job_worker())
            .await
            .expect("enable the runs module as the jobs worker");

        host.submit_at(as_user(2, 2), submit_job("job-ghost", "ghost", "spec"))
            .await
            .expect("unknown agent kind is a no-op for the worker");

        let job = job_view(&host, "job-ghost").await.expect("job exists");
        assert_eq!(job.status, JobStatus::Pending);
        assert!(job.claim.is_none());
        assert_eq!(
            pending_run(&host, &job_run_id_for("job-ghost", "ghost", 2)).await,
            None
        );
    });
}

#[test]
fn a_completed_job_run_finalizes_the_jobs_board_with_the_validated_response() {
    deterministic::Runner::default().start(|context| async move {
        let mut host = genesis(context).await;
        host.submit_at(as_user(1, 1), enable_job_worker())
            .await
            .expect("enable the runs module as the jobs worker");
        host.submit_at(as_user(1, 2), register_duck())
            .await
            .expect("register duck");
        host.submit_at(as_user(2, 3), submit_job("job-1", "duck", "job spec"))
            .await
            .expect("submit job");

        let run_id = job_run_id_for("job-1", "duck", 3);
        let raw = job_response("job-task", "complete job");
        oracle_result(&mut host, &run_id, 10, Ok(wrap_runner(raw.clone()))).await;

        // the oracle block only committed the mailbox; the NEXT block's
        // delivery finalizes the board and emits the task action.
        assert_eq!(job_view(&host, "job-1").await.unwrap().status, JobStatus::Processing);
        host.submit_at(at(11, alice()), noop_block(11))
            .await
            .expect("delivery block");

        assert_eq!(pending_run(&host, &run_id).await, None, "entry pruned");
        let job = job_view(&host, "job-1").await.expect("job exists");
        assert_eq!(job.status, JobStatus::Done);
        let result = job.result.expect("finalize result");
        assert!(result.ok);
        // the finalize payload is a faceted DeliveryReceipt whose `response` is
        // the normalized AgentResponse (a message-only result carries no facets).
        let payload: serde_json::Value =
            serde_json::from_str(&result.payload).expect("delivery receipt json");
        assert_eq!(payload["ducktape_delivery"], 1);
        assert_eq!(payload["status"], "ok");
        let expected: serde_json::Value =
            serde_json::from_slice(&raw).expect("agent response json");
        assert_eq!(
            payload["response"], expected,
            "the normalized response JSON is the finalize payload's response facet"
        );
        assert_eq!(task_ids(&host).await, vec!["job-task".to_string()]);
    });
}

#[test]
fn a_pruned_and_resubmitted_job_id_gets_a_fresh_episode_run() {
    deterministic::Runner::default().start(|context| async move {
        let mut host = genesis(context).await;
        host.submit_at(as_user(1, 1), enable_job_worker())
            .await
            .expect("enable jobs worker");
        host.submit_at(as_user(1, 2), register_duck())
            .await
            .expect("register duck");

        host.submit_at(as_user(2, 3), submit_job("repeat", "duck", "first"))
            .await
            .expect("first submit");
        let first_run_id = job_run_id_for("repeat", "duck", 3);
        oracle_result(
            &mut host,
            &first_run_id,
            10,
            Ok(wrap_runner(job_response("first-task", "finish first"))),
        )
        .await;
        host.submit_at(at(11, alice()), noop_block(11))
            .await
            .expect("first episode delivery");
        assert_eq!(
            job_view(&host, "repeat").await.unwrap().status,
            JobStatus::Done
        );
        host.submit_at(as_user(2, 12), prune_job("repeat"))
            .await
            .expect("prune first episode");

        host.submit_at(as_user(2, 20), submit_job("repeat", "duck", "second"))
            .await
            .expect("resubmit same job id");
        let second_run_id = job_run_id_for("repeat", "duck", 20);
        assert_ne!(first_run_id, second_run_id);
        assert_eq!(
            pending_run(&host, &first_run_id).await,
            None,
            "the delivered first episode left no pending entry behind"
        );
        assert!(
            pending_run(&host, &second_run_id).await.is_some(),
            "fresh entry for the second job episode"
        );

        let job = job_view(&host, "repeat")
            .await
            .expect("resubmitted job exists");
        assert_eq!(job.status, JobStatus::Processing);
        assert_eq!(
            job.claim.as_ref().map(|claim| claim.claimed_at_height),
            Some(20)
        );
    });
}

#[test]
fn a_stale_job_run_does_not_finalize_a_reclaimed_episode() {
    deterministic::Runner::default().start(|context| async move {
        let mut host = genesis(context).await;
        host.submit_at(as_user(1, 1), enable_job_worker())
            .await
            .expect("enable jobs worker");
        host.submit_at(as_user(1, 2), register_duck())
            .await
            .expect("register duck");

        host.submit_at(as_user(2, 3), submit_job("lease", "duck", "stale"))
            .await
            .expect("submit stale episode");
        let stale_run_id = job_run_id_for("lease", "duck", 3);

        host.submit_at(as_user(9, 1004), reclaim_job("lease"))
            .await
            .expect("lease expiry requeues the job");
        host.submit_at(at(1005, Origin::Module("runs".into())), claim_job("lease"))
            .await
            .expect("the runs module reclaims a new episode");

        oracle_result(
            &mut host,
            &stale_run_id,
            1010,
            Ok(wrap_runner(job_response("stale-task", "late stale output"))),
        )
        .await;
        host.submit_at(at(1011, alice()), noop_block(1011))
            .await
            .expect("stale delivery block");

        assert_eq!(
            pending_run(&host, &stale_run_id).await,
            None,
            "the stale entry pruned without finalizing anything"
        );
        let job = job_view(&host, "lease").await.expect("job remains");
        assert_eq!(job.status, JobStatus::Processing);
        assert_eq!(job.result, None);
        assert_eq!(
            job.claim.as_ref().map(|claim| claim.claimed_at_height),
            Some(1005)
        );
    });
}

#[test]
fn a_failed_job_run_finalizes_the_jobs_board_with_error_detail() {
    deterministic::Runner::default().start(|context| async move {
        let mut host = genesis(context).await;
        host.submit_at(as_user(1, 1), enable_job_worker())
            .await
            .expect("enable the runs module as the jobs worker");
        host.submit_at(as_user(1, 2), register_duck())
            .await
            .expect("register duck");
        host.submit_at(as_user(2, 3), submit_job("job-fail", "duck", "job spec"))
            .await
            .expect("submit job");

        let run_id = job_run_id_for("job-fail", "duck", 3);
        let saga_id = dispatch_saga_id(&host, &run_id).await;
        for attempt in 0..2u32 {
            host.submit_at(
                at(
                    10 + u64::from(attempt),
                    Origin::External(b"oracle".to_vec()),
                ),
                Msg {
                    target: "saga".into(),
                    payload: saga_encode_msg(&SagaMsg::OracleResult {
                        saga_id: saga_id.clone(),
                        attempt,
                        outcome: Err("model unavailable".into()),
                        usage: None,
                    }),
                },
            )
            .await
            .expect("failing oracle result");
        }
        host.submit_at(at(20, alice()), noop_block(20))
            .await
            .expect("delivery block");

        assert_eq!(pending_run(&host, &run_id).await, None);
        let job = job_view(&host, "job-fail").await.expect("job exists");
        assert_eq!(job.status, JobStatus::Failed);
        let result = job.result.expect("finalize result");
        assert!(!result.ok);
        assert_eq!(result.payload, "model unavailable");
    });
}

#[test]
fn a_failed_oracle_fails_the_run_and_surfaces_a_failure_reply() {
    deterministic::Runner::default().start(|context| async move {
        let mut host = genesis(context).await;
        for (height, origin, op) in scripted_ops() {
            host.submit_at(at(height, origin), op).await.expect("block");
        }
        let run_id = run_id_for("general", 1, "quackbot");
        let saga_id = dispatch_saga_id(&host, &run_id).await;

        // the oracle reports a hard failure after saga retries are exhausted:
        // the recipe grants one retry (max_attempts = 2), so two failing
        // results land the saga Failed; the dispatch module judges the Err
        // into its mailbox, and the NEXT block's delivery fails the run.
        for attempt in 0..2u32 {
            host.submit_at(
                at(
                    10 + u64::from(attempt),
                    Origin::External(b"oracle".to_vec()),
                ),
                Msg {
                    target: "saga".into(),
                    payload: saga_encode_msg(&SagaMsg::OracleResult {
                        saga_id: saga_id.clone(),
                        attempt,
                        outcome: Err("model unavailable".into()),
                        usage: None,
                    }),
                },
            )
            .await
            .expect("failing oracle block");
        }
        assert_eq!(saga_status(&host, &saga_id).await, Some(SagaStatus::Failed));
        host.submit_at(at(20, alice()), noop_block(20))
            .await
            .expect("delivery block");

        assert_eq!(
            pending_run(&host, &run_id).await,
            None,
            "the failed run's entry pruned"
        );
        // the failure surfaces under the run's ONE reply id, authored as the
        // agent — a real chat post through the same path a success takes.
        let reply = chat_message(&host, &reply_message_id(&run_id))
            .await
            .expect("the failure posted a reply");
        assert_eq!(
            reply.head.author,
            chat::AuthorRef::Agent {
                module: "runs".into(),
                agent_id: "quackbot".into(),
            }
        );
        assert_eq!(
            reply.head.blocks,
            vec![chat::Block::paragraph(
                "⚠ Quackbot failed: model unavailable"
            )]
        );
        assert_eq!(
            task_ids(&host).await,
            Vec::<String>::new(),
            "no task either"
        );
    });
}

#[test]
fn a_response_with_a_disallowed_action_fails_the_run_and_writes_nothing() {
    deterministic::Runner::default().start(|context| async move {
        let mut host = genesis(context).await;
        // same script, but quackbot may ONLY post to chat — no task grants.
        for (height, origin, op) in scripted_ops() {
            let op = if height == 2 {
                register_quackbot(vec![ACTION_CHAT_POST.into()])
            } else {
                op
            };
            host.submit_at(at(height, origin), op).await.expect("block");
        }
        let run_id = run_id_for("general", 1, "quackbot");
        let saga_id = dispatch_saga_id(&host, &run_id).await;

        // the oracle answers with a response that includes a task action the
        // agent was never granted: the delivery block commits (no-fail rule),
        // the run fails deterministically, no task lands — and the failure
        // surfaces as the agent's ⚠ reply (quackbot holds chat.post).
        host.submit_at(
            at(10, Origin::External(b"oracle".to_vec())),
            Msg {
                target: "saga".into(),
                payload: saga_encode_msg(&SagaMsg::OracleResult {
                    saga_id: saga_id.clone(),
                    attempt: 0,
                    outcome: Ok(wrap_runner(canned_response(&run_id))),
                    usage: None,
                }),
            },
        )
        .await
        .expect("the disallowed response must NOT abort the oracle block");
        host.submit_at(at(11, alice()), noop_block(11))
            .await
            .expect("the disallowed response must NOT abort the delivery block");

        assert_eq!(
            pending_run(&host, &run_id).await,
            None,
            "the failed run's entry pruned"
        );
        assert_eq!(
            agent_dispatch(&host, &run_id).await.status,
            DispatchStatus::Delivered,
            "the dispatch itself delivered — the RUN failed, not the block"
        );
        let reply = chat_message(&host, &reply_message_id(&run_id))
            .await
            .expect("the failure posted a reply");
        assert_eq!(
            reply.head.blocks,
            vec![chat::Block::paragraph(
                "⚠ Quackbot failed: agent quackbot is not allowed to tasks.create"
            )]
        );
        assert_eq!(task_ids(&host).await, Vec::<String>::new());
    });
}
