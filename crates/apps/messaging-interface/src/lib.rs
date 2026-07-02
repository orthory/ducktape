//! the messaging module's public wire surface -- types only.
//! writes go via [`MessagingMsg`]; reads via [`MessagingQuery`] ->
//! [`MessagingReply`].

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Channel {
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
pub struct Thread {
    pub root: ChatMessage,
    pub replies: Vec<ChatMessage>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum MessagingMsg {
    CreateChannel {
        channel_id: String,
        name: String,
    },
    PostMessage {
        channel_id: String,
        message_id: String,
        author: String,
        body: String,
    },
    PostThreadReply {
        channel_id: String,
        thread_id: String,
        message_id: String,
        author: String,
        body: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum MessagingQuery {
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
pub enum MessagingReply {
    Channels(Vec<Channel>),
    Channel(Option<Channel>),
    Messages(Vec<ChatMessage>),
    Thread(Option<Thread>),
}

pub fn encode_msg(m: &MessagingMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}

pub fn decode_msg(b: &[u8]) -> Result<MessagingMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_query(q: &MessagingQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}

pub fn decode_query(b: &[u8]) -> Result<MessagingQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_reply(r: &MessagingReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}

pub fn decode_reply(b: &[u8]) -> Result<MessagingReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_message_defaults_thread_metadata_for_older_records() {
        let msg: ChatMessage = serde_json::from_str(
            r#"{
                "id":"m1",
                "channel_id":"general",
                "author":"alice",
                "body":"hello",
                "sequence":1,
                "sent_at":20
            }"#,
        )
        .unwrap();

        assert_eq!(msg.thread_id, None);
        assert_eq!(msg.reply_count, 0);
        assert_eq!(msg.last_reply_at, None);
    }
}
