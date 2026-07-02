//! the per-message-key storage properties: a large channel commits without the
//! old whole-history-value 1 MiB bomb, an oversized body is refused, and the
//! paginated query walks history in bounded windows.

use commonware_runtime::{deterministic, Runner as _};
use messaging::Messaging;
use messaging_interface::{
    decode_reply, encode_msg, encode_query, ChatMessage, MessagingMsg, MessagingQuery,
    MessagingReply,
};
use sdk::{Ctx, Error, Module, Msg, Origin, StateRoot};

struct TestCtx {
    env: sdk::Env,
}
impl TestCtx {
    fn at(t: u64) -> Self {
        Self {
            env: sdk::Env {
                height: 0,
                consensus_time: t,
                origin: Origin::System,
                me: "messaging".into(),
            },
        }
    }
}
#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &sdk::Env {
        &self.env
    }
    fn module_root(&self, _t: &str) -> Option<StateRoot> {
        None
    }
    async fn query(&self, _t: &str, _r: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::QueryUnsupported)
    }
    fn emit_msg(&mut self, _m: Msg) {}
    fn emit_event(&mut self, _e: sdk::Event) {}
    fn request_effect(&mut self, _e: sdk::Effect) {}
}

fn m(payload: MessagingMsg) -> Msg {
    Msg { target: "messaging".into(), payload: encode_msg(&payload) }
}

async fn messages<E>(
    module: &Messaging<E>,
    channel: &str,
    before: Option<u64>,
    limit: Option<u32>,
) -> Vec<ChatMessage>
where
    E: commonware_storage::Context + commonware_runtime::BufferPooler,
{
    let reply = module
        .query(&encode_query(&MessagingQuery::Messages {
            channel_id: channel.into(),
            before,
            limit,
        }))
        .await
        .unwrap();
    match decode_reply(&reply).unwrap() {
        MessagingReply::Messages(v) => v,
        other => panic!("unexpected reply: {other:?}"),
    }
}

#[test]
fn a_large_channel_commits_without_the_whole_history_bomb() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = Messaging::init(context, "messaging").await;
        module
            .execute(&mut TestCtx::at(1), &m(MessagingMsg::CreateChannel {
                channel_id: "general".into(),
                name: "General".into(),
            }))
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        // post FAR more total body bytes than the old whole-channel value could
        // hold (the journal codec bound is 1 MiB): 400 messages of ~4 KiB each
        // is ~1.6 MiB of history. under the old layout the single channel value
        // would blow the codec bound and panic commit_block; per-message keys
        // keep every value tiny, so this commits cleanly.
        let body = "x".repeat(4000);
        for i in 0..400u64 {
            module
                .execute(&mut TestCtx::at(100 + i), &m(MessagingMsg::PostMessage {
                    channel_id: "general".into(),
                    message_id: format!("msg-{i:04}"),
                    author: "bulk".into(),
                    body: body.clone(),
                }))
                .await
                .unwrap();
        }
        module.commit_block().await.unwrap();

        let all = messages(&module, "general", None, None).await;
        assert_eq!(all.len(), 400, "every message committed");
        // ascending sequence, dense from 1.
        for (i, msg) in all.iter().enumerate() {
            assert_eq!(msg.sequence, i as u64 + 1);
            assert_eq!(msg.id, format!("msg-{i:04}"));
        }
    });
}

#[test]
fn an_oversized_body_is_refused() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = Messaging::init(context, "messaging").await;
        module
            .execute(&mut TestCtx::at(1), &m(MessagingMsg::CreateChannel {
                channel_id: "general".into(),
                name: "General".into(),
            }))
            .await
            .unwrap();
        module.commit_block().await.unwrap();

        let huge = "y".repeat(16 * 1024 + 1);
        let err = module
            .execute(&mut TestCtx::at(2), &m(MessagingMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "big".into(),
                author: "a".into(),
                body: huge,
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Module(ref s) if s.contains("ceiling")), "got {err:?}");
        module.abort_block().await.unwrap();
        assert!(messages(&module, "general", None, None).await.is_empty());
    });
}

#[test]
fn pagination_walks_history_newest_first_in_bounded_windows() {
    deterministic::Runner::default().start(|context| async move {
        let mut module = Messaging::init(context, "messaging").await;
        module
            .execute(&mut TestCtx::at(1), &m(MessagingMsg::CreateChannel {
                channel_id: "general".into(),
                name: "General".into(),
            }))
            .await
            .unwrap();
        for i in 0..10u64 {
            module
                .execute(&mut TestCtx::at(10 + i), &m(MessagingMsg::PostMessage {
                    channel_id: "general".into(),
                    message_id: format!("m{i}"),
                    author: "a".into(),
                    body: format!("body {i}"),
                }))
                .await
                .unwrap();
        }
        module.commit_block().await.unwrap();

        // newest page: sequences 10, 9, 8 (newest-first) with limit 3.
        let page = messages(&module, "general", None, Some(3)).await;
        assert_eq!(page.iter().map(|m| m.sequence).collect::<Vec<_>>(), vec![10, 9, 8]);

        // next page BEFORE the oldest seen (8): 7, 6, 5.
        let next = messages(&module, "general", Some(8), Some(3)).await;
        assert_eq!(next.iter().map(|m| m.sequence).collect::<Vec<_>>(), vec![7, 6, 5]);

        // a window past the start clamps, never underflows.
        let head = messages(&module, "general", Some(3), Some(10)).await;
        assert_eq!(head.iter().map(|m| m.sequence).collect::<Vec<_>>(), vec![2, 1]);

        // whole-history read stays ASCENDING (the pre-pagination contract).
        let all = messages(&module, "general", None, None).await;
        assert_eq!(all.iter().map(|m| m.sequence).collect::<Vec<_>>(), (1..=10).collect::<Vec<_>>());
    });
}
