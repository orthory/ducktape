use agent::Agent;
use agent_interface::{
    AgentEntry, AgentMsg, AgentQuery, AgentReply, AgentSession, AgentThread, DEFAULT_AGENT_TARGET,
    DEFAULT_MESSAGING_TARGET, decode_reply, encode_msg, encode_query,
};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use host::Host;
use messaging::Messaging;
use sdk::{Ctx, Error, Module, Msg, Origin, StateRoot};

struct TestCtx {
    env: sdk::Env,
}

impl TestCtx {
    fn at(consensus_time: u64) -> Self {
        Self {
            env: sdk::Env {
                height: 0,
                consensus_time,
                origin: Origin::System,
                me: DEFAULT_AGENT_TARGET.into(),
            },
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &sdk::Env {
        &self.env
    }

    fn module_root(&self, _target: &str) -> Option<StateRoot> {
        None
    }

    async fn query(&self, _target: &str, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::QueryUnsupported)
    }

    fn emit_msg(&mut self, _msg: Msg) {}
    fn emit_event(&mut self, _ev: sdk::Event) {}
    fn request_effect(&mut self, _eff: sdk::Effect) {}
}

fn agent_msg(payload: AgentMsg) -> Msg {
    Msg {
        target: DEFAULT_AGENT_TARGET.into(),
        payload: encode_msg(&payload),
    }
}

fn entry(
    id: &str,
    session_id: &str,
    author: &str,
    body: &str,
    sequence: u64,
    thread_id: Option<&str>,
    reply_count: u64,
    last_reply_at: Option<u64>,
) -> AgentEntry {
    AgentEntry {
        id: id.into(),
        session_id: session_id.into(),
        author: author.into(),
        body: body.into(),
        sequence,
        sent_at: 0,
        thread_id: thread_id.map(str::to_string),
        reply_count,
        last_reply_at,
    }
}

async fn agent_query(host: &Host, query: AgentQuery) -> AgentReply {
    let reply = host
        .query(DEFAULT_AGENT_TARGET, &encode_query(&query))
        .await
        .unwrap();
    decode_reply(&reply).unwrap()
}

async fn apply_commit<E>(module: &mut Agent<E>, at: u64, payload: AgentMsg)
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
    module
        .execute(&mut TestCtx::at(at), &agent_msg(payload))
        .await
        .unwrap();
    module.commit_block().await.unwrap();
}

async fn query_agent<E>(module: &Agent<E>, query: AgentQuery) -> AgentReply
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
    let reply = module.query(&encode_query(&query)).await.unwrap();
    decode_reply(&reply).unwrap()
}

#[test]
fn agent_is_a_queryable_root_backed_session_store() {
    deterministic::Runner::default().start(|context| async move {
        let agent =
            Agent::init_with_messaging_id(context, DEFAULT_AGENT_TARGET, DEFAULT_MESSAGING_TARGET)
                .await;
        let mut host = Host::genesis(vec![Box::new(agent)]).unwrap();
        let root0 = host.module_root(DEFAULT_AGENT_TARGET).unwrap();
        let app0 = host.app_hash();

        let out = host
            .submit(agent_msg(AgentMsg::OpenSession {
                session_id: "s1".into(),
                title: "Planning".into(),
            }))
            .await
            .unwrap();

        assert_ne!(host.module_root(DEFAULT_AGENT_TARGET).unwrap(), root0);
        assert_ne!(out.app_hash, app0);
        assert_eq!(
            agent_query(&host, AgentQuery::Sessions).await,
            AgentReply::Sessions(vec![AgentSession {
                id: "s1".into(),
                title: "Planning".into(),
                created_at: 0,
            }])
        );
        assert_eq!(
            agent_query(
                &host,
                AgentQuery::Session {
                    session_id: "s1".into()
                }
            )
            .await,
            AgentReply::Session(Some(AgentSession {
                id: "s1".into(),
                title: "Planning".into(),
                created_at: 0,
            }))
        );

        host.submit(agent_msg(AgentMsg::AppendMessage {
            session_id: "s1".into(),
            message_id: "m1".into(),
            author: "planner".into(),
            body: "draft the shared context".into(),
        }))
        .await
        .unwrap();

        assert_eq!(
            agent_query(
                &host,
                AgentQuery::Messages {
                    session_id: "s1".into()
                }
            )
            .await,
            AgentReply::Messages(vec![entry(
                "m1",
                "s1",
                "planner",
                "draft the shared context",
                1,
                None,
                0,
                None,
            )])
        );
    });
}

#[test]
fn agent_session_thread_replies_use_backing_threads() {
    deterministic::Runner::default().start(|context| async move {
        let agent =
            Agent::init_with_messaging_id(context, DEFAULT_AGENT_TARGET, DEFAULT_MESSAGING_TARGET)
                .await;
        let mut host = Host::genesis(vec![Box::new(agent)]).unwrap();

        host.submit(agent_msg(AgentMsg::OpenSession {
            session_id: "s1".into(),
            title: "Planning".into(),
        }))
        .await
        .unwrap();
        host.submit(agent_msg(AgentMsg::AppendMessage {
            session_id: "s1".into(),
            message_id: "m1".into(),
            author: "planner".into(),
            body: "draft the shared context".into(),
        }))
        .await
        .unwrap();
        host.submit(agent_msg(AgentMsg::AppendThreadReply {
            session_id: "s1".into(),
            thread_id: "m1".into(),
            message_id: "r1".into(),
            author: "reviewer".into(),
            body: "keep this in the thread".into(),
        }))
        .await
        .unwrap();

        let parent = entry(
            "m1",
            "s1",
            "planner",
            "draft the shared context",
            1,
            None,
            1,
            Some(0),
        );
        let reply = entry(
            "r1",
            "s1",
            "reviewer",
            "keep this in the thread",
            1,
            Some("m1"),
            0,
            None,
        );

        assert_eq!(
            agent_query(
                &host,
                AgentQuery::Messages {
                    session_id: "s1".into()
                }
            )
            .await,
            AgentReply::Messages(vec![parent.clone()])
        );
        assert_eq!(
            agent_query(
                &host,
                AgentQuery::Thread {
                    session_id: "s1".into(),
                    thread_id: "m1".into(),
                }
            )
            .await,
            AgentReply::Thread(Some(AgentThread {
                root: parent,
                replies: vec![reply],
            }))
        );
    });
}

#[test]
fn append_turn_records_user_and_assistant_entries_in_one_block() {
    deterministic::Runner::default().start(|context| async move {
        let agent =
            Agent::init_with_messaging_id(context, DEFAULT_AGENT_TARGET, DEFAULT_MESSAGING_TARGET)
                .await;
        let mut host = Host::genesis(vec![Box::new(agent)]).unwrap();

        host.submit(agent_msg(AgentMsg::OpenSession {
            session_id: "s1".into(),
            title: "Planning".into(),
        }))
        .await
        .unwrap();

        host.submit(agent_msg(AgentMsg::AppendTurn {
            session_id: "s1".into(),
            user_message_id: "u1".into(),
            assistant_message_id: "a1".into(),
            user: "eddy".into(),
            assistant: "codex".into(),
            user_body: "what should the agent module do?".into(),
            assistant_body: "own a queryable session view".into(),
        }))
        .await
        .unwrap();

        assert_eq!(
            agent_query(
                &host,
                AgentQuery::Messages {
                    session_id: "s1".into()
                }
            )
            .await,
            AgentReply::Messages(vec![
                entry(
                    "u1",
                    "s1",
                    "eddy",
                    "what should the agent module do?",
                    1,
                    None,
                    0,
                    None,
                ),
                entry(
                    "a1",
                    "s1",
                    "codex",
                    "own a queryable session view",
                    2,
                    None,
                    0,
                    None,
                ),
            ])
        );
    });
}

#[test]
fn append_turn_rolls_back_if_the_second_backing_write_fails() {
    deterministic::Runner::default().start(|context| async move {
        let agent =
            Agent::init_with_messaging_id(context, DEFAULT_AGENT_TARGET, DEFAULT_MESSAGING_TARGET)
                .await;
        let mut host = Host::genesis(vec![Box::new(agent)]).unwrap();

        host.submit(agent_msg(AgentMsg::OpenSession {
            session_id: "s1".into(),
            title: "Planning".into(),
        }))
        .await
        .unwrap();
        host.submit(agent_msg(AgentMsg::AppendMessage {
            session_id: "s1".into(),
            message_id: "a1".into(),
            author: "codex".into(),
            body: "already answered".into(),
        }))
        .await
        .unwrap();
        let app_hash = host.app_hash();
        let agent_root = host.module_root(DEFAULT_AGENT_TARGET).unwrap();

        let err = host
            .submit(agent_msg(AgentMsg::AppendTurn {
                session_id: "s1".into(),
                user_message_id: "u1".into(),
                assistant_message_id: "a1".into(),
                user: "eddy".into(),
                assistant: "codex".into(),
                user_body: "this staged write must roll back".into(),
                assistant_body: "duplicate id fails the second backing write".into(),
            }))
            .await
            .unwrap_err();

        assert!(matches!(err, sdk::Error::Module(_)));
        assert_eq!(host.app_hash(), app_hash);
        assert_eq!(host.module_root(DEFAULT_AGENT_TARGET).unwrap(), agent_root);
        assert_eq!(
            agent_query(
                &host,
                AgentQuery::Messages {
                    session_id: "s1".into()
                }
            )
            .await,
            AgentReply::Messages(vec![entry(
                "a1",
                "s1",
                "codex",
                "already answered",
                1,
                None,
                0,
                None,
            )])
        );
    });
}

#[test]
fn missing_session_rolls_back_the_agent_backing_store() {
    deterministic::Runner::default().start(|context| async move {
        let agent =
            Agent::init_with_messaging_id(context, DEFAULT_AGENT_TARGET, DEFAULT_MESSAGING_TARGET)
                .await;
        let mut host = Host::genesis(vec![Box::new(agent)]).unwrap();
        let app_hash = host.app_hash();
        let agent_root = host.module_root(DEFAULT_AGENT_TARGET).unwrap();

        let err = host
            .submit(agent_msg(AgentMsg::AppendMessage {
                session_id: "missing".into(),
                message_id: "m1".into(),
                author: "planner".into(),
                body: "nope".into(),
            }))
            .await
            .unwrap_err();

        assert!(matches!(err, sdk::Error::Module(_)));
        assert_eq!(host.app_hash(), app_hash);
        assert_eq!(host.module_root(DEFAULT_AGENT_TARGET).unwrap(), agent_root);
        assert_eq!(
            agent_query(&host, AgentQuery::Sessions).await,
            AgentReply::Sessions(vec![])
        );
    });
}

#[test]
fn synced_agent_reconstructs_the_same_threaded_session_view() {
    deterministic::Runner::default().start(|context| async move {
        let messaging = Messaging::init(context.child("src"), DEFAULT_MESSAGING_TARGET).await;
        let mut src = Agent::from_messaging(DEFAULT_AGENT_TARGET, messaging);

        apply_commit(
            &mut src,
            10,
            AgentMsg::OpenSession {
                session_id: "s1".into(),
                title: "Planning".into(),
            },
        )
        .await;
        apply_commit(
            &mut src,
            20,
            AgentMsg::AppendMessage {
                session_id: "s1".into(),
                message_id: "m1".into(),
                author: "eddy".into(),
                body: "make it richer".into(),
            },
        )
        .await;
        apply_commit(
            &mut src,
            21,
            AgentMsg::AppendThreadReply {
                session_id: "s1".into(),
                thread_id: "m1".into(),
                message_id: "r1".into(),
                author: "codex".into(),
                body: "wrap the backing store".into(),
            },
        )
        .await;

        let src_root = src.root();
        assert_ne!(src_root, StateRoot::ZERO);
        let expected = query_agent(
            &src,
            AgentQuery::Thread {
                session_id: "s1".into(),
                thread_id: "m1".into(),
            },
        )
        .await;
        let target = src.sync_target().await;
        let resolver = src.into_resolver();

        let synced = Agent::sync_from_messaging_id(
            context.child("dst"),
            DEFAULT_AGENT_TARGET,
            DEFAULT_MESSAGING_TARGET,
            target,
            resolver,
        )
        .await;

        assert_eq!(synced.root(), src_root);
        assert_eq!(
            query_agent(
                &synced,
                AgentQuery::Thread {
                    session_id: "s1".into(),
                    thread_id: "m1".into()
                },
            )
            .await,
            expected
        );
    });
}
