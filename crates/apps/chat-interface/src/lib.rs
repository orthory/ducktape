//! Slack-like chat wire surface.
//!
//! Chat is a filtered view over the messaging module's storage model. The
//! public names here match a chat UI while conversion helpers keep the backing
//! persistence surface in `messaging-interface`.

use messaging_interface::{
    Channel as MessagingChannel, ChatMessage as MessagingChatMessage, MessagingMsg, MessagingQuery,
    MessagingReply, Thread as MessagingThread,
};
use serde::{Deserialize, Serialize};

pub const DEFAULT_CHAT_TARGET: &str = "chat";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ChatChannel {
    pub id: String,
    pub name: String,
    pub created_at: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub id: String,
    pub channel_id: String,
    pub author: String,
    pub body: String,
    pub sequence: u64,
    pub sent_at: u64,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub reply_count: u64,
    #[serde(default)]
    pub last_reply_at: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ChatThread {
    pub root: ChatMessage,
    pub replies: Vec<ChatMessage>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ChatMsg {
    CreateChannel {
        channel_id: String,
        name: String,
    },
    SendMessage {
        channel_id: String,
        message_id: String,
        author: String,
        body: String,
    },
    ReplyInThread {
        channel_id: String,
        thread_id: String,
        message_id: String,
        author: String,
        body: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ChatQuery {
    Channels,
    Channel {
        channel_id: String,
    },
    Messages {
        channel_id: String,
    },
    Thread {
        channel_id: String,
        thread_id: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ChatReply {
    Channels(Vec<ChatChannel>),
    Channel(Option<ChatChannel>),
    Messages(Vec<ChatMessage>),
    Thread(Option<ChatThread>),
}

impl From<MessagingChannel> for ChatChannel {
    fn from(channel: MessagingChannel) -> Self {
        Self {
            id: channel.id,
            name: channel.name,
            created_at: channel.created_at,
        }
    }
}

impl From<MessagingChatMessage> for ChatMessage {
    fn from(message: MessagingChatMessage) -> Self {
        Self {
            id: message.id,
            channel_id: message.channel_id,
            author: message.author,
            body: message.body,
            sequence: message.sequence,
            sent_at: message.sent_at,
            thread_id: message.thread_id,
            reply_count: message.reply_count,
            last_reply_at: message.last_reply_at,
        }
    }
}

impl From<MessagingThread> for ChatThread {
    fn from(thread: MessagingThread) -> Self {
        Self {
            root: ChatMessage::from(thread.root),
            replies: thread.replies.into_iter().map(ChatMessage::from).collect(),
        }
    }
}

pub fn backing_msg(msg: ChatMsg) -> MessagingMsg {
    match msg {
        ChatMsg::CreateChannel { channel_id, name } => {
            MessagingMsg::CreateChannel { channel_id, name }
        }
        ChatMsg::SendMessage {
            channel_id,
            message_id,
            author,
            body,
        } => MessagingMsg::PostMessage {
            channel_id,
            message_id,
            author,
            body,
        },
        ChatMsg::ReplyInThread {
            channel_id,
            thread_id,
            message_id,
            author,
            body,
        } => MessagingMsg::PostThreadReply {
            channel_id,
            thread_id,
            message_id,
            author,
            body,
        },
    }
}

pub fn backing_query(query: ChatQuery) -> MessagingQuery {
    match query {
        ChatQuery::Channels => MessagingQuery::Channels,
        ChatQuery::Channel { channel_id } => MessagingQuery::Channel { channel_id },
        ChatQuery::Messages { channel_id } => MessagingQuery::Messages { channel_id },
        ChatQuery::Thread {
            channel_id,
            thread_id,
        } => MessagingQuery::Thread {
            channel_id,
            thread_id,
        },
    }
}

pub fn reply_from_backing(reply: MessagingReply) -> ChatReply {
    match reply {
        MessagingReply::Channels(channels) => {
            ChatReply::Channels(channels.into_iter().map(ChatChannel::from).collect())
        }
        MessagingReply::Channel(channel) => ChatReply::Channel(channel.map(ChatChannel::from)),
        MessagingReply::Messages(messages) => {
            ChatReply::Messages(messages.into_iter().map(ChatMessage::from).collect())
        }
        MessagingReply::Thread(thread) => ChatReply::Thread(thread.map(ChatThread::from)),
    }
}

pub fn encode_msg(m: &ChatMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}

pub fn decode_msg(b: &[u8]) -> Result<ChatMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_query(q: &ChatQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}

pub fn decode_query(b: &[u8]) -> Result<ChatQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_reply(r: &ChatReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}

pub fn decode_reply(b: &[u8]) -> Result<ChatReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
