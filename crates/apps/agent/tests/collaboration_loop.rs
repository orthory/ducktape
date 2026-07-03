//! the collaboration loop end-to-end under a real host + reactor (design §3):
//! a human posts with a mention → chat's hook engages the agent → the run and
//! its saga trigger commit IN THE SAME BLOCK as the post (P2) → the reactor's
//! mock LLM worker answers the WorkerRequest effect as an ordinary oracle op
//! → the saga's terminal transition, its callback, the agent's validated
//! reply (authored as the AGENT), and the task action all commit in ONE block
//! (P2, P6) → and a second composition replaying the identical op sequence
//! (the oracle op included) lands on the byte-identical app-hash — the
//! oracle-as-op laundering that makes a non-deterministic LLM consensus-safe
//! (N2: validators agree on the one finalized output, never on reproducing
//! it).

use std::cell::RefCell;
use std::rc::Rc;

use agent::{
    AgentModule, context_hash, job_run_id_for, job_spec_hash, reply_message_id, run_id_for,
    saga_id_for,
};
use agent_interface::{
    ACTION_CHAT_POST, ACTION_TASKS_CREATE, AgentAction, AgentMsg, AgentOutput, AgentQuery,
    AgentReply, AgentStatus, LlmRequest, RunStatus, TurnPolicy, decode_llm_request, decode_reply,
    encode_msg, encode_output, encode_query,
};
use chat::Chat;
use chat_interface::{
    AuthorRef, Block, ChatMsg, ChatQuery, ChatReply, Mark, PostPolicy, Span,
    decode_reply as chat_decode_reply, encode_msg as chat_encode_msg,
    encode_query as chat_encode_query,
};
use commonware_runtime::{Runner as _, deterministic};
use host::{BlockContext, Host};
use jobs::Jobs;
use jobs_interface::{
    Job, JobStatus, JobsMsg, JobsQuery, JobsReply, decode_reply as jobs_decode_reply,
    encode_msg as jobs_encode_msg, encode_query as jobs_encode_query,
};
use reactor::{Reactor, Worker};
use saga::SagaModule;
use saga_interface::{
    SagaMsg, SagaQuery, SagaReply, SagaStatus, decode_reply as saga_decode_reply,
    decode_worker_request, encode_msg as saga_encode_msg, encode_query as saga_encode_query,
};
use sdk::{Effect, Msg, Origin, StateRoot};
use tasks::Tasks;
use tasks_interface::{
    TaskQuery, TaskReply, decode_reply as tasks_decode_reply, encode_query as tasks_encode_query,
};

/// the mock oracle standing in for the real (non-deterministic) LLM worker:
/// it claims a `WorkerRequest` only when the spec decodes as an [`LlmRequest`]
/// (try-decode routing) and answers with a canned [`AgentOutput`] — a reply
/// plus one task action — through the NORMAL submit path, echoing the
/// request's `(saga_id, attempt)` idempotency key. the call counter lets the
/// test prove how many worker rounds a settle actually took.
struct MockLlmWorker {
    calls: Rc<RefCell<u32>>,
}

#[async_trait::async_trait(?Send)]
impl Worker for MockLlmWorker {
    async fn run(&self, effect: &Effect) -> Result<Option<Msg>, reactor::Error> {
        let Ok(request) = decode_worker_request(&effect.0) else {
            return Ok(None);
        };
        let Ok(llm) = decode_llm_request(&request.spec) else {
            return Ok(None);
        };
        *self.calls.borrow_mut() += 1;
        Ok(Some(Msg {
            target: "saga".into(),
            payload: saga_encode_msg(&SagaMsg::OracleResult {
                saga_id: request.saga_id,
                attempt: request.attempt,
                outcome: Ok(canned_output(&llm.run_id)),
            }),
        }))
    }
}

fn canned_output(run_id: &str) -> Vec<u8> {
    encode_output(&AgentOutput {
        reply_blocks: vec![Block::paragraph(format!("quack: handling {run_id}"))],
        actions: vec![AgentAction::CreateTask {
            task_id: "task-1".into(),
            title: "follow up on the mention".into(),
        }],
    })
}

fn alice() -> Origin {
    Origin::External(b"alice".to_vec())
}

fn at(height: u64, origin: Origin) -> BlockContext {
    BlockContext {
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
        module: "agent".into(),
        agent_id: "quackbot".into(),
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
            Msg {
                target: "agent".into(),
                payload: encode_msg(&AgentMsg::RegisterAgent {
                    agent_id: "quackbot".into(),
                    display_name: "Quackbot".into(),
                    model_ref: "mock-llm-1".into(),
                    prompt_hash: vec![7u8; 32],
                    allowed_actions: vec![ACTION_CHAT_POST.into(), ACTION_TASKS_CREATE.into()],
                }),
            },
        ),
        (
            3,
            alice(),
            Msg {
                target: "agent".into(),
                payload: encode_msg(&AgentMsg::WatchChannel {
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
    let chat = Chat::init(context, "chat").await;
    Host::genesis(vec![
        Box::new(chat),
        Box::new(SagaModule::new("saga")),
        Box::new(AgentModule::new(
            "agent",
            "chat",
            "saga",
            Some("tasks".into()),
            Some("jobs".into()),
        )),
        Box::new(Tasks::new("tasks")),
        Box::new(Jobs::new("jobs")),
    ])
    .expect("genesis")
}

async fn run_view(host: &Host, run_id: &str) -> Option<agent_interface::RunView> {
    let reply = host
        .query(
            "agent",
            &encode_query(&AgentQuery::Run {
                run_id: run_id.into(),
            }),
        )
        .await
        .unwrap();
    match decode_reply(&reply).unwrap() {
        AgentReply::Run(view) => view,
        other => panic!("unexpected reply: {other:?}"),
    }
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
    }
}

async fn chat_message(host: &Host, message_id: &str) -> Option<chat_interface::MessageView> {
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

fn register_worker() -> Msg {
    jobs_msg(JobsMsg::RegisterWorker {
        module_id: "agent".into(),
    })
}

fn submit_job(job_id: &str, agent_id: &str, spec: &str) -> Msg {
    jobs_msg(JobsMsg::Submit {
        job_id: job_id.into(),
        kind: format!("agent/{agent_id}"),
        spec: spec.into(),
    })
}

#[test]
fn a_mention_flows_through_hook_saga_oracle_and_lands_reply_and_task_in_one_block() {
    let run_id = run_id_for("general", 1, "quackbot");
    let replay_run_id = run_id.clone();

    // ---- instance one: the live flow through host + reactor ----------------
    let (settled_hash, oracle_op) = deterministic::Runner::default().start(|context| async move {
        let mut host = genesis(context).await;

        // blocks 1..3: channel, registry, watch. the watch and chat's hook
        // registration are one atomic block (P2).
        let ops = scripted_ops();
        for (height, origin, op) in &ops[..3] {
            host.submit_at(at(*height, origin.clone()), op.clone())
                .await
                .expect("setup block");
        }

        // block 4: the user post. THE SAME BLOCK must carry the message, the
        // hook delivery, the run record, and the saga trigger (P2) — and emit
        // exactly one WorkerRequest effect for the off-consensus seam.
        let (height, origin, post) = &ops[3];
        let outcome = host
            .submit_at(at(*height, origin.clone()), post.clone())
            .await
            .expect("post block");
        let run = run_view(&host, &run_id).await.expect("run created");
        assert_eq!(
            run.status,
            RunStatus::AwaitingOracle {
                saga_id: saga_id_for(&run_id),
            },
            "the run awaits its oracle in the SAME block as the post"
        );
        assert_eq!(
            saga_status(&host, &saga_id_for(&run_id)).await,
            Some(SagaStatus::Pending),
            "the saga is pending in the SAME block too"
        );
        assert_eq!(outcome.effects.len(), 1, "one WorkerRequest effect");

        // the spec is a decodable LlmRequest whose context hash any validator
        // can re-derive from the transcript — verified here out-of-band by
        // querying chat for the pinned window and re-hashing it (P4).
        let request = decode_worker_request(&outcome.effects[0].0).unwrap();
        let llm: LlmRequest = decode_llm_request(&request.spec).unwrap();
        assert_eq!(llm.run_id, run_id);
        assert_eq!(llm.agent_id, "quackbot");
        assert_eq!(llm.model_ref, "mock-llm-1");
        assert_eq!(llm.channel_id, "general");
        assert_eq!(llm.anchor_seq, 1);
        let reply = host
            .query(
                "chat",
                &chat_encode_query(&ChatQuery::MessagesRange {
                    channel_id: "general".into(),
                    from_seq: 1,
                    limit: 1,
                }),
            )
            .await
            .unwrap();
        let ChatReply::Messages(window) = chat_decode_reply(&reply).unwrap() else {
            panic!("messages reply expected");
        };
        assert_eq!(
            llm.context_hash,
            context_hash(&window),
            "the pin re-derives from the log (P4)"
        );

        // nothing downstream exists yet: the reply, the task, and the
        // terminal states all wait on the oracle op.
        assert_eq!(chat_message(&host, &reply_message_id(&run_id)).await, None);
        assert_eq!(task_ids(&host).await, Vec::<String>::new());

        // the mock worker claims the effect and produces the oracle op —
        // capture it so the second instance can replay the IDENTICAL bytes.
        let calls = Rc::new(RefCell::new(0u32));
        let worker = MockLlmWorker {
            calls: calls.clone(),
        };
        let oracle_op = worker
            .run(&outcome.effects[0])
            .await
            .unwrap()
            .expect("the worker claims the LlmRequest effect");
        let calls_before_settle = *calls.borrow();

        // settle through the REACTOR: the oracle op is one block, and its
        // cascade — saga Done + callback + validated reply post + task create
        // — drains inside it (P2, P6). the worker counter staying at zero
        // proves the settle needed no further rounds: everything terminal
        // committed in that ONE block (the reply's own hook notification
        // no-opped under loop prevention instead of spawning a new run).
        let mut reactor = Reactor::new(host, vec![Box::new(worker)]);
        let settled = reactor
            .submit_and_settle(oracle_op.clone())
            .await
            .expect("settle");
        assert_eq!(
            *calls.borrow(),
            calls_before_settle,
            "no worker round during settle -> the terminal cascade was ONE block"
        );

        let host = reactor.host();
        assert_eq!(
            run_view(host, &run_id).await.unwrap().status,
            RunStatus::Done,
            "the run settled Done"
        );
        assert_eq!(
            saga_status(host, &saga_id_for(&run_id)).await,
            Some(SagaStatus::Done),
            "the saga settled Done"
        );
        let reply = chat_message(host, &reply_message_id(&run_id))
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
        assert_eq!(task_ids(host).await, vec!["task-1".to_string()]);

        // loop prevention held: the agent's own reply engaged nothing.
        let runs = host
            .query(
                "agent",
                &encode_query(&AgentQuery::Runs {
                    channel_id: None,
                    limit: 100,
                }),
            )
            .await
            .unwrap();
        let AgentReply::Runs(runs) = decode_reply(&runs).unwrap() else {
            panic!("runs reply expected");
        };
        assert_eq!(runs.len(), 1, "exactly one run — the reply spawned none");

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
        assert_eq!(record.model_ref, "mock-llm-1");

        assert_eq!(settled.app_hash, reactor.app_hash());
        (settled.app_hash, oracle_op)
    });

    // ---- instance two: replay the identical op sequence -------------------
    // a fresh composition applies the same four blocks plus the SAME oracle
    // op (the reactor submits worker results through `Host::submit`, i.e. the
    // default block context — mirrored here) and must land on the
    // byte-identical app-hash: the LLM's non-determinism was laundered into
    // an ordered op, so replay is pure state-machine (N2).
    let replayed_hash = deterministic::Runner::default().start(|context| async move {
        let mut host = genesis(context).await;
        for (height, origin, op) in scripted_ops() {
            host.submit_at(at(height, origin), op)
                .await
                .expect("replayed block");
        }
        let outcome = host.submit(oracle_op).await.expect("replayed oracle op");
        assert_eq!(
            run_view(&host, &replay_run_id).await.unwrap().status,
            RunStatus::Done
        );
        outcome.app_hash
    });

    assert_eq!(
        settled_hash, replayed_hash,
        "two instances, one op sequence -> byte-identical app-hash"
    );
    assert_ne!(settled_hash, StateRoot::ZERO);
}

#[test]
fn an_agent_job_is_claimed_and_records_a_run_in_the_submit_cascade() {
    deterministic::Runner::default().start(|context| async move {
        let mut host = genesis(context).await;
        host.submit_at(as_user(1, 1), register_worker())
            .await
            .expect("register the agent module as the single jobs worker");
        host.submit_at(
            as_user(1, 2),
            Msg {
                target: "agent".into(),
                payload: encode_msg(&AgentMsg::RegisterAgent {
                    agent_id: "duck".into(),
                    display_name: "Duck".into(),
                    model_ref: "mock-llm-1".into(),
                    prompt_hash: vec![9u8; 32],
                    allowed_actions: vec![ACTION_TASKS_CREATE.into()],
                }),
            },
        )
        .await
        .expect("register duck");

        let spec = "summarize this work item";
        host.submit_at(as_user(2, 3), submit_job("job-1", "duck", spec))
            .await
            .expect("submit cascades to agent claim + run");

        let job = job_view(&host, "job-1").await.expect("job exists");
        assert_eq!(job.status, JobStatus::Processing);
        assert_eq!(
            job.claim.as_ref().map(|claim| claim.worker.as_str()),
            Some("agent")
        );

        let run_id = job_run_id_for("job-1", "duck");
        let run = run_view(&host, &run_id).await.expect("job run exists");
        assert_eq!(run.job_id, Some("job-1".into()));
        assert_eq!(run.agent_id, "duck");
        assert_eq!(run.context_hash, job_spec_hash(spec.as_bytes()));
        assert_eq!(
            run.status,
            RunStatus::AwaitingOracle {
                saga_id: saga_id_for(&run_id),
            }
        );
    });
}

#[test]
fn an_agent_job_for_an_unknown_agent_commits_without_a_claim() {
    deterministic::Runner::default().start(|context| async move {
        let mut host = genesis(context).await;
        host.submit_at(as_user(1, 1), register_worker())
            .await
            .expect("register the agent module as the jobs worker");

        host.submit_at(as_user(2, 2), submit_job("job-ghost", "ghost", "spec"))
            .await
            .expect("unknown agent kind is a no-op for the worker");

        let job = job_view(&host, "job-ghost").await.expect("job exists");
        assert_eq!(job.status, JobStatus::Pending);
        assert!(job.claim.is_none());
        assert_eq!(
            run_view(&host, &job_run_id_for("job-ghost", "ghost")).await,
            None
        );
    });
}

#[test]
fn a_completed_job_run_finalizes_the_jobs_board_with_the_validated_output() {
    deterministic::Runner::default().start(|context| async move {
        let mut host = genesis(context).await;
        host.submit_at(as_user(1, 1), register_worker())
            .await
            .expect("register the agent module as the jobs worker");
        host.submit_at(
            as_user(1, 2),
            Msg {
                target: "agent".into(),
                payload: encode_msg(&AgentMsg::RegisterAgent {
                    agent_id: "duck".into(),
                    display_name: "Duck".into(),
                    model_ref: "mock-llm-1".into(),
                    prompt_hash: vec![9u8; 32],
                    allowed_actions: vec![ACTION_TASKS_CREATE.into()],
                }),
            },
        )
        .await
        .expect("register duck");
        host.submit_at(as_user(2, 3), submit_job("job-1", "duck", "job spec"))
            .await
            .expect("submit job");

        let run_id = job_run_id_for("job-1", "duck");
        let output = AgentOutput {
            reply_blocks: Vec::new(),
            actions: vec![AgentAction::CreateTask {
                task_id: "job-task".into(),
                title: "complete job".into(),
            }],
        };
        host.submit_at(
            at(10, Origin::External(b"oracle".to_vec())),
            Msg {
                target: "saga".into(),
                payload: saga_encode_msg(&SagaMsg::OracleResult {
                    saga_id: saga_id_for(&run_id),
                    attempt: 0,
                    outcome: Ok(encode_output(&output)),
                }),
            },
        )
        .await
        .expect("oracle result finalizes the job");

        assert_eq!(
            run_view(&host, &run_id).await.unwrap().status,
            RunStatus::Done
        );
        let job = job_view(&host, "job-1").await.expect("job exists");
        assert_eq!(job.status, JobStatus::Done);
        let result = job.result.expect("finalize result");
        assert!(result.ok);
        assert_eq!(
            result.payload,
            String::from_utf8(encode_output(&output)).expect("json output is utf8")
        );
        assert_eq!(task_ids(&host).await, vec!["job-task".to_string()]);
    });
}

#[test]
fn a_failed_job_run_finalizes_the_jobs_board_with_error_detail() {
    deterministic::Runner::default().start(|context| async move {
        let mut host = genesis(context).await;
        host.submit_at(as_user(1, 1), register_worker())
            .await
            .expect("register the agent module as the jobs worker");
        host.submit_at(
            as_user(1, 2),
            Msg {
                target: "agent".into(),
                payload: encode_msg(&AgentMsg::RegisterAgent {
                    agent_id: "duck".into(),
                    display_name: "Duck".into(),
                    model_ref: "mock-llm-1".into(),
                    prompt_hash: vec![9u8; 32],
                    allowed_actions: vec![ACTION_TASKS_CREATE.into()],
                }),
            },
        )
        .await
        .expect("register duck");
        host.submit_at(as_user(2, 3), submit_job("job-fail", "duck", "job spec"))
            .await
            .expect("submit job");

        let run_id = job_run_id_for("job-fail", "duck");
        for attempt in 0..2u32 {
            host.submit_at(
                at(
                    10 + u64::from(attempt),
                    Origin::External(b"oracle".to_vec()),
                ),
                Msg {
                    target: "saga".into(),
                    payload: saga_encode_msg(&SagaMsg::OracleResult {
                        saga_id: saga_id_for(&run_id),
                        attempt,
                        outcome: Err("model unavailable".into()),
                    }),
                },
            )
            .await
            .expect("failing oracle result");
        }

        assert_eq!(
            run_view(&host, &run_id).await.unwrap().status,
            RunStatus::Failed {
                reason: "model unavailable".into(),
            }
        );
        let job = job_view(&host, "job-fail").await.expect("job exists");
        assert_eq!(job.status, JobStatus::Failed);
        let result = job.result.expect("finalize result");
        assert!(!result.ok);
        assert_eq!(result.payload, "model unavailable");
    });
}

#[test]
fn a_failed_oracle_fails_the_run_without_any_follow_ups() {
    deterministic::Runner::default().start(|context| async move {
        let mut host = genesis(context).await;
        for (height, origin, op) in scripted_ops() {
            host.submit_at(at(height, origin), op).await.expect("block");
        }
        let run_id = run_id_for("general", 1, "quackbot");

        // the oracle reports a hard failure after saga retries are exhausted:
        // saga v2 retries an Err while attempts remain (max_attempts = 2), so
        // two failing results land the saga — and therefore the run — Failed,
        // all in the failing result's own block (P6).
        for attempt in 0..2u32 {
            host.submit_at(
                at(
                    10 + u64::from(attempt),
                    Origin::External(b"oracle".to_vec()),
                ),
                Msg {
                    target: "saga".into(),
                    payload: saga_encode_msg(&SagaMsg::OracleResult {
                        saga_id: saga_id_for(&run_id),
                        attempt,
                        outcome: Err("model unavailable".into()),
                    }),
                },
            )
            .await
            .expect("failing oracle block");
        }

        assert_eq!(
            run_view(&host, &run_id).await.unwrap().status,
            RunStatus::Failed {
                reason: "model unavailable".into(),
            }
        );
        assert_eq!(
            saga_status(&host, &saga_id_for(&run_id)).await,
            Some(SagaStatus::Failed)
        );
        assert_eq!(
            chat_message(&host, &reply_message_id(&run_id)).await,
            None,
            "no reply was ever posted"
        );
        assert_eq!(
            task_ids(&host).await,
            Vec::<String>::new(),
            "no task either"
        );
    });
}

#[test]
fn an_output_with_a_disallowed_action_fails_the_run_and_writes_nothing() {
    deterministic::Runner::default().start(|context| async move {
        let mut host = genesis(context).await;
        // same script, but quackbot may ONLY post to chat — no task grants.
        for (height, origin, op) in scripted_ops() {
            let op = if height == 2 {
                Msg {
                    target: "agent".into(),
                    payload: encode_msg(&AgentMsg::RegisterAgent {
                        agent_id: "quackbot".into(),
                        display_name: "Quackbot".into(),
                        model_ref: "mock-llm-1".into(),
                        prompt_hash: vec![7u8; 32],
                        allowed_actions: vec![ACTION_CHAT_POST.into()],
                    }),
                }
            } else {
                op
            };
            host.submit_at(at(height, origin), op).await.expect("block");
        }
        let run_id = run_id_for("general", 1, "quackbot");

        // the oracle answers with an output that includes a task action the
        // agent was never granted: the block commits (no-fail rule), the run
        // fails deterministically, and NOTHING was written to chat or tasks.
        host.submit_at(
            at(10, Origin::External(b"oracle".to_vec())),
            Msg {
                target: "saga".into(),
                payload: saga_encode_msg(&SagaMsg::OracleResult {
                    saga_id: saga_id_for(&run_id),
                    attempt: 0,
                    outcome: Ok(canned_output(&run_id)),
                }),
            },
        )
        .await
        .expect("the disallowed output must NOT abort the block");

        let RunStatus::Failed { reason } = run_view(&host, &run_id).await.unwrap().status else {
            panic!("the run must fail");
        };
        assert!(reason.contains(ACTION_TASKS_CREATE));
        assert_eq!(
            saga_status(&host, &saga_id_for(&run_id)).await,
            Some(SagaStatus::Done),
            "the saga itself still settled — the RUN failed, not the block"
        );
        assert_eq!(chat_message(&host, &reply_message_id(&run_id)).await, None);
        assert_eq!(task_ids(&host).await, Vec::<String>::new());
    });
}
