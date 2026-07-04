//! the probe layer against the REAL chat module: a squatted deterministic
//! message id and a missing action channel are downgraded to run records
//! instead of aborting the posting user's block, while the happy path still
//! posts — all through chat's genuine hook fan-out, not a stand-in.

use automations::Automations;
use automations_interface::{
    Action, AutomationsMsg, AutomationsQuery, AutomationsReply, RunRecord, Trigger,
    decode_reply as auto_decode_reply, encode_msg as auto_encode_msg,
    encode_query as auto_encode_query,
};
use chat::Chat;
use chat_interface::{
    AuthorRef, Block, ChatMsg, ChatQuery, ChatReply, MessageView, PostPolicy,
    decode_reply as chat_decode_reply, encode_msg as chat_encode_msg,
    encode_query as chat_encode_query,
};
use commonware_runtime::{Runner as _, deterministic};
use host::{BlockContext, Host};
use sdk::{Msg, Origin};
use tasks::Tasks;

const AUTO: &str = "automations";
const CHAT: &str = "chat";
const TASKS: &str = "tasks";
const INBOX: &str = "inbox";
const MEMORY: &str = "memory";

fn as_user(byte: u8, height: u64) -> BlockContext {
    BlockContext { protocol_version: 0,
        height,
        consensus_time: height * 100,
        origin: Origin::External(vec![byte; 32]),
    }
}

fn chat_msg(payload: ChatMsg) -> Msg {
    Msg {
        target: CHAT.into(),
        payload: chat_encode_msg(&payload),
    }
}

fn post(channel: &str, message_id: &str, text: &str) -> Msg {
    chat_msg(ChatMsg::PostMessage {
        channel_id: channel.into(),
        message_id: message_id.into(),
        blocks: vec![Block::paragraph(text)],
        thread: None,
        as_agent: None,
    })
}

fn create_rule(rule_id: &str, channel: &str, action: Action) -> Msg {
    Msg {
        target: AUTO.into(),
        payload: auto_encode_msg(&AutomationsMsg::CreateRule {
            rule_id: rule_id.into(),
            trigger: Trigger::MessagePosted {
                channel_id: Some(channel.into()),
                mention: None,
                text_contains: None,
            },
            action,
        }),
    }
}

async fn messages(host: &Host, channel: &str) -> Vec<MessageView> {
    let reply = host
        .query(
            CHAT,
            &chat_encode_query(&ChatQuery::MessagesLatest {
                channel_id: channel.into(),
                limit: 64,
            }),
        )
        .await
        .expect("chat query");
    match chat_decode_reply(&reply).expect("chat reply") {
        ChatReply::Messages(views) => views,
        other => panic!("expected Messages, got {other:?}"),
    }
}

async fn run_history(host: &Host, rule_id: &str) -> Vec<RunRecord> {
    let reply = host
        .query(
            AUTO,
            &auto_encode_query(&AutomationsQuery::RunHistory {
                rule_id: rule_id.into(),
                limit: 16,
            }),
        )
        .await
        .expect("auto query");
    match auto_decode_reply(&reply).expect("auto reply") {
        AutomationsReply::History(records) => records,
        other => panic!("expected History, got {other:?}"),
    }
}

/// genesis a real chat + tasks + automations host with channel "general"
/// created, the automations hook registered on it, and one rule installed.
async fn arena(context: deterministic::Context, rule_id: &str, action: Action) -> Host {
    let chat = Chat::init(context, CHAT).await;
    let mut host = Host::genesis(vec![
        Box::new(chat),
        Box::new(Tasks::new(TASKS)),
        Box::new(Automations::new(AUTO, CHAT, TASKS, INBOX, MEMORY)),
    ])
    .expect("genesis");

    host.submit_at(
        as_user(1, 1),
        chat_msg(ChatMsg::CreateChannel {
            channel_id: "general".into(),
            name: "General".into(),
            post_policy: PostPolicy::Open,
        }),
    )
    .await
    .expect("create channel");
    host.submit_at(
        as_user(1, 2),
        chat_msg(ChatMsg::RegisterHook {
            channel_id: "general".into(),
            module_id: AUTO.into(),
        }),
    )
    .await
    .expect("register hook");
    host.submit_at(as_user(1, 3), create_rule(rule_id, "general", action))
        .await
        .expect("create rule");
    host
}

#[test]
fn squatted_message_id_downgrades_to_run_record() {
    deterministic::Runner::default().start(|context| async move {
        let mut host = arena(
            context,
            "echo",
            Action::PostMessage {
                channel_id: "general".into(),
                template: "echo {seq}".into(),
            },
        )
        .await;

        // the squat IS the trigger: the user's post claims the exact id the
        // rule will compose for its own seq ("auto-echo-general-1"). without
        // the probe, the emitted follow-up would hit chat's duplicate-id
        // rejection and abort the user's posting block.
        host.submit_at(
            as_user(2, 4),
            post("general", "auto-echo-general-1", "squat"),
        )
        .await
        .expect("the squatted fire must not abort the posting block");

        let views = messages(&host, "general").await;
        assert_eq!(views.len(), 1, "only the user's post landed");
        assert_eq!(views[0].head.message_id, "auto-echo-general-1");
        assert!(matches!(views[0].head.author, AuthorRef::User(_)));

        let recs = run_history(&host, "echo").await;
        assert_eq!(recs.len(), 1);
        assert!(!recs[0].action_ok);
        assert!(recs[0].detail.contains("already taken"));

        // a normal post fires cleanly: the rule's reply lands as a module-
        // authored message, whose own hook delivery is a loop-prevention no-op.
        host.submit_at(as_user(2, 5), post("general", "m2", "hello"))
            .await
            .expect("normal fire");
        let views = messages(&host, "general").await;
        assert_eq!(views.len(), 3, "user post + rule reply");
        assert_eq!(views[2].head.message_id, "auto-echo-general-2");
        assert_eq!(views[2].head.author, AuthorRef::Module(AUTO.into()));
        assert_eq!(
            views[2].head.blocks,
            vec![Block::paragraph("echo 2")],
            "placeholders substituted from the triggering event"
        );

        let recs = run_history(&host, "echo").await;
        assert_eq!(recs.len(), 2);
        assert!(recs[1].action_ok);
    });
}

#[test]
fn missing_action_channel_downgrades_to_run_record() {
    deterministic::Runner::default().start(|context| async move {
        let mut host = arena(
            context,
            "ghostly",
            Action::PostMessage {
                channel_id: "ghost".into(),
                template: "boo".into(),
            },
        )
        .await;

        // "ghost" was never created: without the probe, chat would reject the
        // emitted post ("unknown channel") and abort the user's block.
        host.submit_at(as_user(2, 4), post("general", "m1", "hello"))
            .await
            .expect("a missing action channel must not abort the posting block");

        let views = messages(&host, "general").await;
        assert_eq!(views.len(), 1, "the user's post committed");

        let recs = run_history(&host, "ghostly").await;
        assert_eq!(recs.len(), 1);
        assert!(!recs[0].action_ok);
        assert!(recs[0].detail.contains("does not exist"));
    });
}
