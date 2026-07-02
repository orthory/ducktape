//! in-memory messaging module with explicit channels.
//!
//! the module stores channels and per-channel messages as replicated state. like
//! directory and valset, it uses the host-lent staging seam: `execute` validates
//! and stages writes, `query` reads committed state plus the pending overlay, and
//! `commit_block` publishes the block atomically.

use std::collections::{BTreeMap, BTreeSet};

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

    fn snapshot_of(
        channels: &BTreeMap<String, Channel>,
        messages: &BTreeMap<String, ChatMessage>,
    ) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(channels.len() as u64).to_le_bytes());
        for channel in channels.values() {
            write_string(&mut out, &channel.id);
            write_string(&mut out, &channel.name);
            out.extend_from_slice(&channel.created_at.to_le_bytes());
        }
        out.extend_from_slice(&(messages.len() as u64).to_le_bytes());
        for message in messages.values() {
            write_string(&mut out, &message.id);
            write_string(&mut out, &message.channel_id);
            write_string(&mut out, &message.author);
            write_string(&mut out, &message.body);
            out.extend_from_slice(&message.sequence.to_le_bytes());
            out.extend_from_slice(&message.sent_at.to_le_bytes());
        }
        out
    }

    fn root_of(
        channels: &BTreeMap<String, Channel>,
        messages: &BTreeMap<String, ChatMessage>,
    ) -> StateRoot {
        if channels.is_empty() && messages.is_empty() {
            return StateRoot::ZERO;
        }
        StateRoot(Sha256::digest(Self::snapshot_of(channels, messages)).into())
    }

    // ---- state-sync ---------------------------------------------------------
    // A snapshot is committed state only. The serving peer is untrusted; install
    // must rederive the expected root before mutating local state.

    pub fn snapshot(&self) -> Vec<u8> {
        Self::snapshot_of(&self.channels, &self.messages)
    }

    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let (channels, messages) = Self::decode_snapshot(bytes)?;
        let root = Self::root_of(&channels, &messages);
        if root != expected {
            return Err(Error::Module(format!(
                "snapshot root mismatch: decoded {root:?}, expected {expected:?}"
            )));
        }
        self.channels = channels;
        self.messages = messages;
        self.pending = Pending::default();
        Ok(())
    }

    fn decode_snapshot(
        bytes: &[u8],
    ) -> Result<(BTreeMap<String, Channel>, BTreeMap<String, ChatMessage>), Error> {
        let mut off = 0usize;
        let channel_count = read_u64(bytes, &mut off)?;
        if channel_count > ((bytes.len() - off) / 24) as u64 {
            return Err(Error::Module("snapshot truncated".into()));
        }

        let mut channels = BTreeMap::new();
        for _ in 0..channel_count {
            let id = read_string(bytes, &mut off)?;
            let name = read_string(bytes, &mut off)?;
            let created_at = read_u64(bytes, &mut off)?;
            Self::validate_non_empty("channel_id", &id)?;
            Self::validate_non_empty("name", &name)?;
            if channels
                .last_key_value()
                .is_some_and(|(last, _)| *last >= id)
            {
                return Err(Error::Module(
                    "snapshot channel ids not strictly ascending".into(),
                ));
            }
            channels.insert(
                id.clone(),
                Channel {
                    id,
                    name,
                    created_at,
                },
            );
        }

        let message_count = read_u64(bytes, &mut off)?;
        if message_count > ((bytes.len() - off) / 48) as u64 {
            return Err(Error::Module("snapshot truncated".into()));
        }

        let mut messages = BTreeMap::new();
        let mut channel_sequences: BTreeMap<String, BTreeSet<u64>> = BTreeMap::new();
        for _ in 0..message_count {
            let id = read_string(bytes, &mut off)?;
            let channel_id = read_string(bytes, &mut off)?;
            let author = read_string(bytes, &mut off)?;
            let body = read_string(bytes, &mut off)?;
            let sequence = read_u64(bytes, &mut off)?;
            let sent_at = read_u64(bytes, &mut off)?;

            Self::validate_non_empty("message_id", &id)?;
            Self::validate_non_empty("channel_id", &channel_id)?;
            Self::validate_non_empty("author", &author)?;
            if !channels.contains_key(&channel_id) {
                return Err(Error::Module(format!(
                    "snapshot message references unknown channel: {channel_id}"
                )));
            }
            if sequence == 0 {
                return Err(Error::Module(
                    "snapshot message sequence must be positive".into(),
                ));
            }
            if messages
                .last_key_value()
                .is_some_and(|(last, _)| *last >= id)
            {
                return Err(Error::Module(
                    "snapshot message ids not strictly ascending".into(),
                ));
            }
            if !channel_sequences
                .entry(channel_id.clone())
                .or_default()
                .insert(sequence)
            {
                return Err(Error::Module(format!(
                    "snapshot duplicate sequence {sequence} in channel {channel_id}"
                )));
            }

            messages.insert(
                id.clone(),
                ChatMessage {
                    id,
                    channel_id,
                    author,
                    body,
                    sequence,
                    sent_at,
                },
            );
        }

        if off != bytes.len() {
            return Err(Error::Module("snapshot has trailing bytes".into()));
        }
        for (channel_id, sequences) in channel_sequences {
            for (idx, sequence) in sequences.into_iter().enumerate() {
                let expected = idx as u64 + 1;
                if sequence != expected {
                    return Err(Error::Module(format!(
                        "snapshot channel {channel_id} has non-contiguous sequence {sequence}, expected {expected}"
                    )));
                }
            }
        }
        Ok((channels, messages))
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
        Self::root_of(&self.channels, &self.messages)
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

fn write_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn read_u64(bytes: &[u8], off: &mut usize) -> Result<u64, Error> {
    let end = off
        .checked_add(8)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| Error::Module("snapshot truncated".into()))?;
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[*off..end]);
    *off = end;
    Ok(u64::from_le_bytes(buf))
}

fn read_string(bytes: &[u8], off: &mut usize) -> Result<String, Error> {
    let len = read_u64(bytes, off)?;
    let len = usize::try_from(len).map_err(|_| Error::Module("snapshot truncated".into()))?;
    if len > bytes.len() - *off {
        return Err(Error::Module("snapshot truncated".into()));
    }
    let s = std::str::from_utf8(&bytes[*off..*off + len])
        .map_err(|_| Error::Module("snapshot string is not utf-8".into()))?;
    *off += len;
    Ok(s.to_owned())
}
