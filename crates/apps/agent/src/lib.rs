//! Shared agent-session command facade over the messaging module.
//!
//! Agent owns no durable state. It translates session-level commands into typed
//! `messaging` follow-up ops, so the storage root and state-sync surface stay in
//! the messaging module.

use agent_interface::{AgentMsg, decode_msg};
use messaging_interface::{MessagingMsg, encode_msg as encode_messaging_msg};
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot};

pub struct Agent {
    id: ModuleId,
    messaging: ModuleId,
}

impl Agent {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            messaging: agent_interface::DEFAULT_MESSAGING_TARGET.into(),
        }
    }

    pub fn with_messaging(id: impl Into<ModuleId>, messaging: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            messaging: messaging.into(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Agent {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            AgentMsg::OpenSession { session_id, title } => ctx.emit_msg(Msg {
                target: self.messaging.clone(),
                payload: encode_messaging_msg(&MessagingMsg::CreateChannel {
                    channel_id: session_id,
                    name: title,
                }),
            }),
            AgentMsg::AppendMessage {
                session_id,
                message_id,
                author,
                body,
            } => ctx.emit_msg(Msg {
                target: self.messaging.clone(),
                payload: encode_messaging_msg(&MessagingMsg::PostMessage {
                    channel_id: session_id,
                    message_id,
                    author,
                    body,
                }),
            }),
            AgentMsg::AppendThreadReply {
                session_id,
                thread_id,
                message_id,
                author,
                body,
            } => ctx.emit_msg(Msg {
                target: self.messaging.clone(),
                payload: encode_messaging_msg(&MessagingMsg::PostThreadReply {
                    channel_id: session_id,
                    thread_id,
                    message_id,
                    author,
                    body,
                }),
            }),
        }
        Ok(())
    }
}
