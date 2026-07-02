use agent::Agent;
use agent_interface::{AgentMsg, DEFAULT_MESSAGING_TARGET, encode_msg};
use commonware_runtime::{Runner as _, deterministic};
use host::Host;
use messaging::Messaging;
use messaging_interface::{
    Channel, ChatMessage, MessagingQuery, MessagingReply, decode_reply, encode_query,
};
use sdk::{Msg, StateRoot};

fn agent_msg(payload: AgentMsg) -> Msg {
    Msg {
        target: agent_interface::DEFAULT_AGENT_TARGET.into(),
        payload: encode_msg(&payload),
    }
}

async fn messaging_query(host: &Host, query: MessagingQuery) -> MessagingReply {
    let reply = host
        .query(DEFAULT_MESSAGING_TARGET, &encode_query(&query))
        .await
        .unwrap();
    decode_reply(&reply).unwrap()
}

#[test]
fn agent_session_commands_drive_messaging_backing_state() {
    deterministic::Runner::default().start(|context| async move {
        let messaging = Messaging::init(context, DEFAULT_MESSAGING_TARGET).await;
        let agent = Agent::new(agent_interface::DEFAULT_AGENT_TARGET);
        let mut host = Host::genesis(vec![Box::new(messaging), Box::new(agent)]).unwrap();
        let agent_root = host
            .module_root(agent_interface::DEFAULT_AGENT_TARGET)
            .unwrap();
        let messaging_root = host.module_root(DEFAULT_MESSAGING_TARGET).unwrap();

        host.submit(agent_msg(AgentMsg::OpenSession {
            session_id: "s1".into(),
            title: "Planning".into(),
        }))
        .await
        .unwrap();

        assert_eq!(
            host.module_root(agent_interface::DEFAULT_AGENT_TARGET)
                .unwrap(),
            agent_root,
            "agent is a stateless shared-session facade"
        );
        assert_ne!(
            host.module_root(DEFAULT_MESSAGING_TARGET).unwrap(),
            messaging_root,
            "agent command must move messaging state"
        );
        assert_eq!(
            messaging_query(&host, MessagingQuery::Channels).await,
            MessagingReply::Channels(vec![Channel {
                id: "s1".into(),
                name: "Planning".into(),
                created_at: 0,
            }])
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
            messaging_query(
                &host,
                MessagingQuery::Messages {
                    channel_id: "s1".into()
                }
            )
            .await,
            MessagingReply::Messages(vec![ChatMessage {
                id: "m1".into(),
                channel_id: "s1".into(),
                author: "planner".into(),
                body: "draft the shared context".into(),
                sequence: 1,
                sent_at: 0,
            }])
        );
    });
}

#[test]
fn missing_session_rolls_back_the_agent_block() {
    deterministic::Runner::default().start(|context| async move {
        let messaging = Messaging::init(context, DEFAULT_MESSAGING_TARGET).await;
        let agent = Agent::new(agent_interface::DEFAULT_AGENT_TARGET);
        let mut host = Host::genesis(vec![Box::new(messaging), Box::new(agent)]).unwrap();
        let app_hash = host.app_hash();
        let messaging_root = host.module_root(DEFAULT_MESSAGING_TARGET).unwrap();

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
        assert_eq!(
            host.module_root(DEFAULT_MESSAGING_TARGET).unwrap(),
            messaging_root
        );
        assert_eq!(
            host.module_root(agent_interface::DEFAULT_AGENT_TARGET)
                .unwrap(),
            StateRoot::ZERO
        );
    });
}
