//! Agent-session wire surface.
//!
//! Agent presents a session-oriented API while keeping durable storage in the
//! messaging model. The conversion helpers are part of the interface so clients
//! and the module agree on the exact mapping.

use messaging_interface::{
    Channel as MessagingChannel, ChatMessage as MessagingChatMessage, MessagingMsg, MessagingQuery,
    MessagingReply, Thread as MessagingThread,
};
use serde::{Deserialize, Serialize};

pub const DEFAULT_AGENT_TARGET: &str = "agent";
pub const DEFAULT_MESSAGING_TARGET: &str = "messaging";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AgentSession {
    pub id: String,
    pub title: String,
    pub created_at: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AgentEntry {
    pub id: String,
    pub session_id: String,
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
pub struct AgentThread {
    pub root: AgentEntry,
    pub replies: Vec<AgentEntry>,
}

pub type AgentSessionMessage = AgentEntry;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum AgentMsg {
    OpenSession {
        session_id: String,
        title: String,
    },
    AppendMessage {
        session_id: String,
        message_id: String,
        author: String,
        body: String,
    },
    AppendTurn {
        session_id: String,
        user_message_id: String,
        assistant_message_id: String,
        user: String,
        assistant: String,
        user_body: String,
        assistant_body: String,
    },
    AppendThreadReply {
        session_id: String,
        thread_id: String,
        message_id: String,
        author: String,
        body: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum AgentQuery {
    Sessions,
    Session {
        session_id: String,
    },
    Messages {
        session_id: String,
    },
    Thread {
        session_id: String,
        thread_id: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum AgentReply {
    Sessions(Vec<AgentSession>),
    Session(Option<AgentSession>),
    Messages(Vec<AgentEntry>),
    Thread(Option<AgentThread>),
}

impl From<MessagingChannel> for AgentSession {
    fn from(channel: MessagingChannel) -> Self {
        Self {
            id: channel.id,
            title: channel.name,
            created_at: channel.created_at,
        }
    }
}

impl From<MessagingChatMessage> for AgentEntry {
    fn from(message: MessagingChatMessage) -> Self {
        Self {
            id: message.id,
            session_id: message.channel_id,
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

impl From<MessagingThread> for AgentThread {
    fn from(thread: MessagingThread) -> Self {
        Self {
            root: AgentEntry::from(thread.root),
            replies: thread.replies.into_iter().map(AgentEntry::from).collect(),
        }
    }
}

pub fn backing_msgs(msg: AgentMsg) -> Vec<MessagingMsg> {
    match msg {
        AgentMsg::OpenSession { session_id, title } => {
            vec![MessagingMsg::CreateChannel {
                channel_id: session_id,
                name: title,
            }]
        }
        AgentMsg::AppendMessage {
            session_id,
            message_id,
            author,
            body,
        } => vec![MessagingMsg::PostMessage {
            channel_id: session_id,
            message_id,
            author,
            body,
        }],
        AgentMsg::AppendTurn {
            session_id,
            user_message_id,
            assistant_message_id,
            user,
            assistant,
            user_body,
            assistant_body,
        } => vec![
            MessagingMsg::PostMessage {
                channel_id: session_id.clone(),
                message_id: user_message_id,
                author: user,
                body: user_body,
            },
            MessagingMsg::PostMessage {
                channel_id: session_id,
                message_id: assistant_message_id,
                author: assistant,
                body: assistant_body,
            },
        ],
        AgentMsg::AppendThreadReply {
            session_id,
            thread_id,
            message_id,
            author,
            body,
        } => vec![MessagingMsg::PostThreadReply {
            channel_id: session_id,
            thread_id,
            message_id,
            author,
            body,
        }],
    }
}

pub fn backing_query(query: AgentQuery) -> MessagingQuery {
    match query {
        AgentQuery::Sessions => MessagingQuery::Channels,
        AgentQuery::Session { session_id } => MessagingQuery::Channel {
            channel_id: session_id,
        },
        AgentQuery::Messages { session_id } => MessagingQuery::Messages {
            channel_id: session_id,
        },
        AgentQuery::Thread {
            session_id,
            thread_id,
        } => MessagingQuery::Thread {
            channel_id: session_id,
            thread_id,
        },
    }
}

pub fn reply_from_backing(reply: MessagingReply) -> AgentReply {
    match reply {
        MessagingReply::Channels(channels) => {
            AgentReply::Sessions(channels.into_iter().map(AgentSession::from).collect())
        }
        MessagingReply::Channel(channel) => AgentReply::Session(channel.map(AgentSession::from)),
        MessagingReply::Messages(messages) => {
            AgentReply::Messages(messages.into_iter().map(AgentEntry::from).collect())
        }
        MessagingReply::Thread(thread) => AgentReply::Thread(thread.map(AgentThread::from)),
    }
}

pub fn encode_msg(m: &AgentMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}

pub fn decode_msg(b: &[u8]) -> Result<AgentMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_query(q: &AgentQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}

pub fn decode_query(b: &[u8]) -> Result<AgentQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_reply(r: &AgentReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}

pub fn decode_reply(b: &[u8]) -> Result<AgentReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
