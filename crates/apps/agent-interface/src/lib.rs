//! Shared agent-session wire surface.
//!
//! `AgentMsg` targets the `agent` module, which translates session-level
//! commands into the storage-backed `messaging` module. Reads go directly to
//! `messaging` for now: the current SDK query method has no `Ctx`, so a
//! stateless facade cannot query another module during `query`.

use serde::{Deserialize, Serialize};

pub const DEFAULT_AGENT_TARGET: &str = "agent";
pub const DEFAULT_MESSAGING_TARGET: &str = "messaging";

pub type AgentSession = messaging_interface::Channel;
pub type AgentSessionMessage = messaging_interface::ChatMessage;

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
    AppendThreadReply {
        session_id: String,
        thread_id: String,
        message_id: String,
        author: String,
        body: String,
    },
}

pub fn encode_msg(m: &AgentMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}

pub fn decode_msg(b: &[u8]) -> Result<AgentMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
