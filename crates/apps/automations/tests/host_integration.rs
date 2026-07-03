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
use inbox::Inbox;
use inbox_interface::{
    InboxQuery, InboxReply, decode_reply as inbox_decode_reply, encode_query as inbox_encode_query,
};
use memory::Memory;
use memory_interface::{MemoryMsg, Meta, PublishBody, encode_msg as memory_encode_msg};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};
use tasks::Tasks;
use tasks_interface::{
    TaskQuery, TaskReply, decode_reply as tasks_decode_reply, encode_query as tasks_encode_query,
};

const AUTO: &str = "automations";
const CHAT: &str = "chat";
const TASKS: &str = "tasks";
const INBOX: &str = "inbox";
const MEMORY: &str = "memory";
const FILES: &str = "files";

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

async fn inbox_items(host: &Host, member: &str) -> Vec<inbox_interface::Notification> {
    let bytes = host
        .query(
            INBOX,
            &inbox_encode_query(&InboxQuery::List {
                member: member.into(),
                from_seq: 0,
                limit: 16,
            }),
        )
        .await
        .expect("query inbox");
    match inbox_decode_reply(&bytes).expect("inbox reply") {
        InboxReply::Items(items) => items,
        other => panic!("expected Items, got {other:?}"),
    }
}

fn genesis() -> Host {
    Host::genesis(vec![
        Box::new(Tasks::new(TASKS)),
        Box::new(RelayChat),
        Box::new(Inbox::new(INBOX)),
        Box::new(Memory::new(MEMORY, FILES)),
        Box::new(Automations::new(AUTO, CHAT, TASKS, INBOX, MEMORY)),
    ])
    .expect("genesis")
}

fn memory_msg(payload: MemoryMsg) -> Msg {
    Msg {
        target: MEMORY.into(),
        payload: memory_encode_msg(&payload),
    }
}

#[test]
fn memory_publish_fires_rule_and_delivers_inbox_atomically() {
    block_on(async {
        let mut host = genesis();

        let (ctx, msg) = from_user(create_rule_msg(
            "memory-inbox",
            Trigger::MemoryPublished {
                prefix: Some("/docs".into()),
                meta_kind: Some("decision".into()),
                author_contains: None,
            },
            Action::DeliverInbox {
                member_template: "mem-{author}".into(),
                kind: "memory-watch".into(),
                body_template:
                    "path={path} generation={generation} author={author} channel=[{channel}]".into(),
            },
        ));
        host.submit_at(ctx, msg).await.expect("create rule");

        host.submit_at(
            BlockContext {
                height: 2,
                consensus_time: 200,
                origin: Origin::External(b"operator".to_vec()),
            },
            memory_msg(MemoryMsg::RegisterWatch {
                prefix: "/docs".into(),
                module_id: AUTO.into(),
            }),
        )
        .await
        .expect("register watch");

        let app_before = host.app_hash();
        let mut meta = Meta::new();
        meta.insert("kind".into(), "decision".into());
        let out = host
            .submit_at(
                BlockContext {
                    height: 3,
                    consensus_time: 300,
                    origin: Origin::External(vec![0xaa]),
                },
                memory_msg(MemoryMsg::Publish {
                    path: "/docs/a".into(),
                    body: PublishBody::Inline("body".into()),
                    meta,
                }),
            )
            .await
            .expect("publish fires");
        assert_ne!(
            out.app_hash, app_before,
            "the same block moved the app hash"
        );

        let items = inbox_items(&host, "mem-ext:aa").await;
        assert_eq!(items.len(), 1, "one notification landed");
        assert_eq!(items[0].kind, "memory-watch");
        assert_eq!(
            items[0].body,
            "path=/docs/a generation=1 author=ext:aa channel=[]"
        );
        assert_eq!(items[0].source, AUTO, "deliver came from automations");

        let recs = run_history(&host, "memory-inbox").await;
        assert_eq!(recs.len(), 1);
        assert!(recs[0].action_ok);
        assert_eq!(recs[0].channel_id, "/docs/a");
        assert_eq!(recs[0].seq, 1);
    });
}

#[test]
fn memory_event_authored_by_automations_does_not_fire() {
    block_on(async {
        let mut host = genesis();
        let (ctx, msg) = from_user(create_rule_msg(
            "memory-loop",
            Trigger::MemoryPublished {
                prefix: Some("/".into()),
                meta_kind: None,
                author_contains: None,
            },
            Action::DeliverInbox {
                member_template: "alice".into(),
                kind: "memory-watch".into(),
                body_template: "path={path}".into(),
            },
        ));
        host.submit_at(ctx, msg).await.expect("create rule");

        host.submit_at(
            BlockContext {
                height: 2,
                consensus_time: 200,
                origin: Origin::External(b"operator".to_vec()),
            },
            memory_msg(MemoryMsg::RegisterWatch {
                prefix: "/".into(),
                module_id: AUTO.into(),
            }),
        )
        .await
        .expect("register watch");

        host.submit_at(
            BlockContext {
                height: 3,
                consensus_time: 300,
                origin: Origin::Module(AUTO.into()),
            },
            memory_msg(MemoryMsg::Publish {
                path: "/docs/self".into(),
                body: PublishBody::Inline("body".into()),
                meta: Meta::new(),
            }),
        )
        .await
        .expect("self-authored publish commits");

        assert!(
            inbox_items(&host, "alice").await.is_empty(),
            "no notification from self-authored memory event"
        );
        assert!(
            run_history(&host, "memory-loop").await.is_empty(),
            "loop-guarded event leaves no run record"
        );
    });
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
fn squatted_task_id_is_caught_by_probe_and_block_commits() {
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

        // squat the deterministic id the next fire will compose: without the
        // probe, tasks would reject the duplicate and abort the posting block.
        host.submit_at(
            BlockContext {
                height: 2,
                consensus_time: 200,
                origin: Origin::External(b"squatter".to_vec()),
            },
            Msg {
                target: TASKS.into(),
                payload: tasks_interface::encode_msg(&tasks_interface::TaskMsg::CreateTask {
                    task_id: "auto-general-5".into(),
                    title: "squatted".into(),
                }),
            },
        )
        .await
        .expect("squat the id");

        // the fire is downgraded to a run record; the user's post commits.
        host.submit_at(
            BlockContext {
                height: 3,
                consensus_time: 300,
                origin: Origin::External(b"poster".to_vec()),
            },
            chat_event_msg("general", 5, AuthorRef::User(vec![1; 4])),
        )
        .await
        .expect("the squatted fire must not abort the block");
        assert_eq!(tasks_of(&host).await.len(), 1, "only the squatter's task");
        let recs = run_history(&host, "capture").await;
        assert_eq!(recs.len(), 1);
        assert!(!recs[0].action_ok);
        assert!(recs[0].detail.contains("already exists"));
    });
}

#[test]
fn post_probe_collision_still_aborts_the_block() {
    block_on(async {
        let mut host = genesis();
        // two rules composing the SAME task id fire on one event: both probes
        // run before either follow-up applies, so both pass and both emit —
        // the second follow-up then fails at tasks and the whole block aborts
        // (P2). the probe layer is best-effort by design; atomicity is the
        // backstop.
        for rule_id in ["r1", "r2"] {
            let (ctx, msg) = from_user(create_rule_msg(
                rule_id,
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
        }
        let app_before = host.app_hash();

        let err = host
            .submit_at(
                BlockContext {
                    height: 2,
                    consensus_time: 200,
                    origin: Origin::External(b"poster".to_vec()),
                },
                chat_event_msg("general", 5, AuthorRef::User(vec![1; 4])),
            )
            .await
            .expect_err("the same-event id collision aborts the block");
        assert!(
            matches!(err, host::SubmitError::Rejected(Error::Module(ref m)) if m.contains("already exists")),
            "unexpected error: {err:?}"
        );
        assert_eq!(
            host.app_hash(),
            app_before,
            "the aborted block left the app-hash untouched"
        );
        assert!(tasks_of(&host).await.is_empty(), "no task landed");
        assert!(
            run_history(&host, "r1").await.is_empty(),
            "aborted records leave no trace"
        );
    });
}
