use host::Host;
use messaging::Messaging;
use messaging_interface::{
    ChatMessage, MessagingMsg, MessagingQuery, MessagingReply, decode_reply, encode_msg,
    encode_query,
};
use sdk::Msg;

fn msg(payload: MessagingMsg) -> Msg {
    Msg {
        target: "messaging".into(),
        payload: encode_msg(&payload),
    }
}

#[test]
fn host_commits_channel_messages_and_serves_history_queries() {
    futures::executor::block_on(async {
        let mut host = Host::genesis(vec![Box::new(Messaging::new("messaging"))]).unwrap();
        let root0 = host.module_root("messaging").unwrap();
        let app0 = host.app_hash();

        let out1 = host
            .submit(msg(MessagingMsg::CreateChannel {
                channel_id: "general".into(),
                name: "General".into(),
            }))
            .await
            .unwrap();
        assert_ne!(host.module_root("messaging").unwrap(), root0);
        assert_ne!(out1.app_hash, app0);

        host.submit(msg(MessagingMsg::PostMessage {
            channel_id: "general".into(),
            message_id: "m1".into(),
            author: "alice".into(),
            body: "hello".into(),
        }))
        .await
        .unwrap();

        let reply = host
            .query(
                "messaging",
                &encode_query(&MessagingQuery::Messages {
                    channel_id: "general".into(),
                }),
            )
            .await
            .unwrap();

        assert_eq!(
            decode_reply(&reply).unwrap(),
            MessagingReply::Messages(vec![ChatMessage {
                id: "m1".into(),
                channel_id: "general".into(),
                author: "alice".into(),
                body: "hello".into(),
                sequence: 1,
                sent_at: 0,
            }])
        );
    });
}

#[test]
fn host_rolls_back_failed_message_blocks() {
    futures::executor::block_on(async {
        let mut host = Host::genesis(vec![Box::new(Messaging::new("messaging"))]).unwrap();
        let root0 = host.module_root("messaging").unwrap();
        let app0 = host.app_hash();

        let err = host
            .submit(msg(MessagingMsg::PostMessage {
                channel_id: "missing".into(),
                message_id: "m1".into(),
                author: "alice".into(),
                body: "hello".into(),
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, sdk::Error::Module(_)));
        assert_eq!(host.module_root("messaging").unwrap(), root0);
        assert_eq!(host.app_hash(), app0);
    });
}
