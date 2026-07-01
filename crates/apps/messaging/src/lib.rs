//! in-memory messaging module with explicit channels.
//!
//! the module stores channels and per-channel messages as replicated state. like
//! directory and valset, it uses the host-lent staging seam: `execute` validates
//! and stages writes, `query` reads committed state plus the pending overlay, and
//! `commit_block` publishes the block atomically.

use std::collections::BTreeMap;

use messaging_interface::{
    Channel, ChatMessage, MessagingMsg, MessagingQuery, MessagingReply, decode_msg, decode_query,
    encode_reply,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot};
use sha2::{Digest, Sha256};

#[derive(Default)]
struct Pending {
    channels: BTreeMap<String, Channel>,
    messages: BTreeMap<String, ChatMessage>,
}

pub struct Messaging {
    id: ModuleId,
    channels: BTreeMap<String, Channel>,
    messages: BTreeMap<String, ChatMessage>,
    pending: Pending,
}

impl Messaging {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            channels: BTreeMap::new(),
            messages: BTreeMap::new(),
            pending: Pending::default(),
        }
    }

    fn validate_non_empty(field: &str, value: &str) -> Result<(), Error> {
        if value.is_empty() {
            return Err(Error::Module(format!("{field} must not be empty")));
        }
        Ok(())
    }

    fn channel(&self, channel_id: &str) -> Option<Channel> {
        self.pending
            .channels
            .get(channel_id)
            .or_else(|| self.channels.get(channel_id))
            .cloned()
    }

    fn channels(&self) -> Vec<Channel> {
        let mut channels = self.channels.clone();
        channels.extend(self.pending.channels.clone());
        channels.into_values().collect()
    }

    fn message_exists(&self, message_id: &str) -> bool {
        self.messages.contains_key(message_id) || self.pending.messages.contains_key(message_id)
    }

    fn messages(&self, channel_id: &str) -> Vec<ChatMessage> {
        let mut messages: Vec<ChatMessage> = self
            .messages
            .values()
            .chain(self.pending.messages.values())
            .filter(|m| m.channel_id == channel_id)
            .cloned()
            .collect();
        messages.sort_by(|a, b| a.sequence.cmp(&b.sequence).then_with(|| a.id.cmp(&b.id)));
        messages
    }

    fn next_sequence(&self, channel_id: &str) -> u64 {
        self.messages(channel_id)
            .last()
            .map_or(1, |m| m.sequence + 1)
    }

    fn stage_channel(
        &mut self,
        channel_id: String,
        name: String,
        created_at: u64,
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", &channel_id)?;
        Self::validate_non_empty("name", &name)?;
        if self.channel(&channel_id).is_some() {
            return Err(Error::Module(format!(
                "channel already exists: {channel_id}"
            )));
        }
        self.pending.channels.insert(
            channel_id.clone(),
            Channel {
                id: channel_id,
                name,
                created_at,
            },
        );
        Ok(())
    }

    fn stage_message(
        &mut self,
        channel_id: String,
        message_id: String,
        author: String,
        body: String,
        sent_at: u64,
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", &channel_id)?;
        Self::validate_non_empty("message_id", &message_id)?;
        Self::validate_non_empty("author", &author)?;
        if self.channel(&channel_id).is_none() {
            return Err(Error::Module(format!("unknown channel: {channel_id}")));
        }
        if self.message_exists(&message_id) {
            return Err(Error::Module(format!(
                "message already exists: {message_id}"
            )));
        }
        let sequence = self.next_sequence(&channel_id);
        self.pending.messages.insert(
            message_id.clone(),
            ChatMessage {
                id: message_id,
                channel_id,
                author,
                body,
                sequence,
                sent_at,
            },
        );
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Messaging {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        if self.channels.is_empty() && self.messages.is_empty() {
            return StateRoot::ZERO;
        }

        let mut h = Sha256::new();
        h.update(b"ducktape.messaging.v1");
        h.update((self.channels.len() as u64).to_le_bytes());
        for channel in self.channels.values() {
            hash_str(&mut h, &channel.id);
            hash_str(&mut h, &channel.name);
            h.update(channel.created_at.to_le_bytes());
        }
        h.update((self.messages.len() as u64).to_le_bytes());
        for message in self.messages.values() {
            hash_str(&mut h, &message.id);
            hash_str(&mut h, &message.channel_id);
            hash_str(&mut h, &message.author);
            hash_str(&mut h, &message.body);
            h.update(message.sequence.to_le_bytes());
            h.update(message.sent_at.to_le_bytes());
        }
        StateRoot(h.finalize().into())
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            MessagingMsg::CreateChannel { channel_id, name } => {
                self.stage_channel(channel_id, name, ctx.env().consensus_time)
            }
            MessagingMsg::PostMessage {
                channel_id,
                message_id,
                author,
                body,
            } => self.stage_message(
                channel_id,
                message_id,
                author,
                body,
                ctx.env().consensus_time,
            ),
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            MessagingQuery::Channels => {
                Ok(encode_reply(&MessagingReply::Channels(self.channels())))
            }
            MessagingQuery::Channel { channel_id } => Ok(encode_reply(&MessagingReply::Channel(
                self.channel(&channel_id),
            ))),
            MessagingQuery::Messages { channel_id } => Ok(encode_reply(&MessagingReply::Messages(
                self.messages(&channel_id),
            ))),
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        self.channels.append(&mut self.pending.channels);
        self.messages.append(&mut self.pending.messages);
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending = Pending::default();
        Ok(())
    }
}

fn hash_str(h: &mut Sha256, value: &str) {
    h.update((value.len() as u64).to_le_bytes());
    h.update(value.as_bytes());
}
