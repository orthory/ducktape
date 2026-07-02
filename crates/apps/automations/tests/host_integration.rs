//! integration: the real host routes a chat hook follow-up into automations with
//! `Origin::Module("chat")`, a rule fires, and its `CreateTask` follow-up lands
//! in the real tasks module — all atomically within one block.

use automations::Automations;
use automations_interface::{
    Action, AutomationsMsg, AutomationsQuery, AutomationsReply, RunRecord, Trigger, decode_reply,
    encode_msg, encode_query,
};
use chat_interface::{AuthorRef, ChatEvent, encode_event};
use futures::executor::block_on;
use host::{BlockContext, Host};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};
use tasks::Tasks;
use tasks_interface::{
    TaskQuery, TaskReply, decode_reply as tasks_decode_reply, encode_query as tasks_encode_query,
};

const AUTO: &str = "automations";
const CHAT: &str = "chat";
const TASKS: &str = "tasks";

/// a stand-in for chat that relays its payload to automations as a hook
/// follow-up. because it is registered under the id "chat", the host stamps the
/// follow-up with `Origin::Module("chat")` — exactly what automations trusts.
struct RelayChat;

#[async_trait::async_trait(?Send)]
impl Module for RelayChat {
    fn id(&self) -> ModuleId {
        CHAT.into()
    }
    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }
    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        ctx.emit_msg(Msg {
            target: AUTO.into(),
            payload: msg.payload.clone(),
        });
        Ok(())
    }
}

fn from_user(payload: Msg) -> (BlockContext, Msg) {
    (
        BlockContext {
            height: 1,
            consensus_time: 100,
            origin: Origin::External(b"operator".to_vec()),
        },
        payload,
    )
}

fn create_rule_msg(rule_id: &str, trigger: Trigger, action: Action) -> Msg {
    Msg {
        target: AUTO.into(),
        payload: encode_msg(&AutomationsMsg::CreateRule {
            rule_id: rule_id.into(),
            trigger,
            action,
        }),
    }
}

fn chat_event_msg(channel: &str, seq: u64, author: AuthorRef) -> Msg {
    Msg {
        target: CHAT.into(),
        payload: encode_event(&ChatEvent::MessagePosted {
            channel_id: channel.into(),
            seq,
            thread_root: None,
            author,
            mentions: Vec::new(),
        }),
    }
}

async fn tasks_of(host: &Host) -> Vec<tasks_interface::Task> {
    let bytes = host
        .query(TASKS, &tasks_encode_query(&TaskQuery::List))
        .await
        .expect("query");
    match tasks_decode_reply(&bytes).expect("reply") {
        TaskReply::Tasks(tasks) => tasks,
    }
}

async fn run_history(host: &Host, rule_id: &str) -> Vec<RunRecord> {
    let bytes = host
        .query(
            AUTO,
            &encode_query(&AutomationsQuery::RunHistory {
                rule_id: rule_id.into(),
                limit: 16,
            }),
        )
        .await
        .expect("query");
    match decode_reply(&bytes).expect("reply") {
        AutomationsReply::History(records) => records,
        other => panic!("expected History, got {other:?}"),
    }
}

fn genesis() -> Host {
    Host::genesis(vec![
        Box::new(Tasks::new(TASKS)),
        Box::new(RelayChat),
        Box::new(Automations::new(AUTO, CHAT, TASKS)),
    ])
    .expect("genesis")
}

#[test]
fn user_post_fires_rule_and_creates_task_atomically() {
    block_on(async {
        let mut host = genesis();

        // register a CreateTask rule.
        let (ctx, msg) = from_user(create_rule_msg(
            "capture",
            Trigger::MessagePosted {
                channel_id: Some("general".into()),
                mention: None,
                text_contains: None,
            },
            Action::CreateTask {
                task_id_prefix: "auto".into(),
                title_template: "post {seq} in {channel}".into(),
            },
        ));
        host.submit_at(ctx, msg).await.expect("create rule");

        // a user post in "general" flows chat -> automations -> tasks in one block.
        let app_before = host.app_hash();
        let out = host
            .submit_at(
                BlockContext {
                    height: 2,
                    consensus_time: 200,
                    origin: Origin::External(b"poster".to_vec()),
                },
                chat_event_msg("general", 5, AuthorRef::User(vec![1; 4])),
            )
            .await
            .expect("hook fires");
        assert_ne!(out.app_hash, app_before, "the fire moved the app-hash");

        let tasks = tasks_of(&host).await;
        assert_eq!(tasks.len(), 1, "the rule created exactly one task");
        assert_eq!(tasks[0].id, "auto-general-5", "deterministic task id");
        assert_eq!(tasks[0].title, "post 5 in general");

        let recs = run_history(&host, "capture").await;
        assert_eq!(recs.len(), 1);
        assert!(recs[0].action_ok);
        assert_eq!(recs[0].seq, 5);
    });
}

#[test]
fn module_authored_post_does_not_fire() {
    block_on(async {
        let mut host = genesis();
        let (ctx, msg) = from_user(create_rule_msg(
            "capture",
            Trigger::MessagePosted {
                channel_id: None,
                mention: None,
                text_contains: None,
            },
            Action::CreateTask {
                task_id_prefix: "auto".into(),
                title_template: "T".into(),
            },
        ));
        host.submit_at(ctx, msg).await.expect("create rule");

        // a module-authored post (loop-prevention target) must not fire.
        host.submit_at(
            BlockContext {
                height: 2,
                consensus_time: 200,
                origin: Origin::External(b"poster".to_vec()),
            },
            chat_event_msg("general", 1, AuthorRef::Module("automations".into())),
        )
        .await
        .expect("no-fail arm");

        assert!(
            tasks_of(&host).await.is_empty(),
            "no task from a module post"
        );
    });
}

#[test]
fn emitted_followup_failure_aborts_the_whole_block() {
    block_on(async {
        let mut host = genesis();
        // target a task_id_prefix such that a second identical fire collides at
        // the tasks module (duplicate task id) — that follow-up failure must
        // abort the entire posting block (P2), leaving no trace.
        let (ctx, msg) = from_user(create_rule_msg(
            "dupe",
            Trigger::MessagePosted {
                channel_id: None,
                mention: None,
                text_contains: None,
            },
            Action::CreateTask {
                task_id_prefix: "auto".into(),
                title_template: "T".into(),
            },
        ));
        host.submit_at(ctx, msg).await.expect("create rule");

        // first fire creates auto-general-5.
        host.submit_at(
            BlockContext {
                height: 2,
                consensus_time: 200,
                origin: Origin::External(b"p".to_vec()),
            },
            chat_event_msg("general", 5, AuthorRef::User(vec![1; 4])),
        )
        .await
        .expect("first fire");
        let app_after_first = host.app_hash();

        // a second post at the same seq re-derives the same task id -> tasks
        // rejects the duplicate -> the block aborts.
        let err = host
            .submit_at(
                BlockContext {
                    height: 3,
                    consensus_time: 300,
                    origin: Origin::External(b"p".to_vec()),
                },
                chat_event_msg("general", 5, AuthorRef::User(vec![2; 4])),
            )
            .await
            .expect_err("duplicate task id aborts the block");
        assert!(
            matches!(err, host::SubmitError::Rejected(Error::Module(ref m)) if m.contains("already exists")),
            "unexpected error: {err:?}"
        );
        assert_eq!(
            host.app_hash(),
            app_after_first,
            "the aborted block left the app-hash untouched"
        );
        assert_eq!(tasks_of(&host).await.len(), 1, "no second task landed");
    });
}
