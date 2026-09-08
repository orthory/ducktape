//! the key space and its typed record IO.
//!
//! every record is its OWN store key, so an op touches only the keys it names
//! and `root()` stays the store's cached merkle root. logical keys are built
//! with [`sdk::wire::encode`] over a `(prefix, parts…)` tuple: the encoding is
//! injective, so no pair of ids can collide across two compound families the
//! way a `prefix + id + "/" + id` concatenation can.
//!
//! | key | value |
//! |---|---|
//! | `("p", pid)` | [`Participant`] |
//! | `("c", cid)` | [`Conversation`] (roster inline, bounded) |
//! | `("b", cid, pid)` | [`Binding`] |
//! | `("e", cid, seq)` | [`ConversationEvent`] — the committed event stream |
//! | `("m", cid, seq)` | [`Message`] — immutable once written |
//! | `("r", cid, seq)` | [`Receipt`] — the one mutable per-recipient record |
//! | `("x", pid, credential, sequence)` | dedup record [`Admission`] |
//! | `("f", pid, credential)` | that credential's replay floor (u64) |
//! | `("q", pid)` | [`MailboxUsage`] |
//! | `("s", recipient, sender)` | that sender's undelivered count in that mailbox |

use sdk::{Error, StagedStore};
use serde::{Serialize, de::DeserializeOwned};

use crate::interface::{
    Binding, Conversation, ConversationEvent, Credential, MailboxUsage, Message, Participant,
    Receipt, MAX_ENCODED_MESSAGE_BYTES,
};

/// write-time cap on ONE stored record, mirroring `tasks::MAX_RECORD_BYTES`:
/// the concrete store's codec bounds a stored value at 1 MiB AT DECODE TIME,
/// so an oversized value would commit fine and then panic every later read on
/// every validator. the 4 KiB margin covers the operation framing.
pub const MAX_RECORD_BYTES: usize = (1 << 20) - 4 * 1024;

/// what one admitted [`crate::interface::MessageId`] resolved to.
///
/// it is retained for exactly as long as its message is: a
/// [`crate::interface::CollaborationMsg::Prune`] drops the body and this
/// record together and raises that credential's replay floor past the
/// sequence. so the retry window IS the retention window — after it, the
/// answer is [`crate::interface::SendState::ReceiptPruned`] off the floor
/// alone, which needs no per-message record and cannot become a second
/// admission.
#[derive(Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    pub conversation_id: String,
    pub seq: u64,
    /// lowercase hex sha256 over the canonical send request.
    pub digest: String,
}

fn key(parts: &impl Serialize) -> Vec<u8> {
    sdk::wire::encode(parts)
}

pub fn participant_key(pid: &str) -> Vec<u8> {
    key(&("p", pid))
}
pub fn conversation_key(cid: &str) -> Vec<u8> {
    key(&("c", cid))
}
pub fn binding_key(cid: &str, pid: &str) -> Vec<u8> {
    key(&("b", cid, pid))
}
pub fn event_key(cid: &str, seq: u64) -> Vec<u8> {
    key(&("e", cid, seq))
}
pub fn message_key(cid: &str, seq: u64) -> Vec<u8> {
    key(&("m", cid, seq))
}
pub fn receipt_key(cid: &str, seq: u64) -> Vec<u8> {
    key(&("r", cid, seq))
}
pub fn admission_key(pid: &str, credential: Credential, sequence: u64) -> Vec<u8> {
    key(&("x", pid, credential, sequence))
}
pub fn replay_floor_key(pid: &str, credential: Credential) -> Vec<u8> {
    key(&("f", pid, credential))
}
pub fn mailbox_key(pid: &str) -> Vec<u8> {
    key(&("q", pid))
}
pub fn sender_quota_key(recipient: &str, sender: &str) -> Vec<u8> {
    key(&("s", recipient, sender))
}

async fn load<T: DeserializeOwned>(
    staged: &StagedStore,
    key: &[u8],
    what: &str,
) -> Result<Option<T>, Error> {
    let Some(bytes) = staged.get(key).await? else {
        return Ok(None);
    };
    sdk::wire::decode(&bytes)
        .map(Some)
        .map_err(|e| Error::Module(format!("{what} record decode: {e}")))
}

/// refuse a value the store's codec would later panic decoding. every op that
/// writes several records CHECKS them all before staging any, so a refused op
/// leaves no overlay entry at all — the invariant that keeps the native module
/// and its wasm port root-identical (see `tasks::check_record`).
pub fn check_record(value: &[u8], what: &str) -> Result<(), Error> {
    if value.len() > MAX_RECORD_BYTES {
        return Err(Error::Module(format!(
            "{what} is {} bytes, over the {MAX_RECORD_BYTES}-byte store record cap",
            value.len()
        )));
    }
    Ok(())
}

pub async fn participant(staged: &StagedStore, pid: &str) -> Result<Option<Participant>, Error> {
    load(staged, &participant_key(pid), "participant").await
}

pub async fn conversation(staged: &StagedStore, cid: &str) -> Result<Option<Conversation>, Error> {
    load(staged, &conversation_key(cid), "conversation").await
}

pub async fn binding(
    staged: &StagedStore,
    cid: &str,
    pid: &str,
) -> Result<Option<Binding>, Error> {
    load(staged, &binding_key(cid, pid), "binding").await
}

pub async fn event(
    staged: &StagedStore,
    cid: &str,
    seq: u64,
) -> Result<Option<ConversationEvent>, Error> {
    load(staged, &event_key(cid, seq), "event").await
}

pub async fn message(staged: &StagedStore, cid: &str, seq: u64) -> Result<Option<Message>, Error> {
    load(staged, &message_key(cid, seq), "message").await
}

pub async fn receipt(staged: &StagedStore, cid: &str, seq: u64) -> Result<Option<Receipt>, Error> {
    load(staged, &receipt_key(cid, seq), "receipt").await
}

pub async fn admission(
    staged: &StagedStore,
    pid: &str,
    credential: Credential,
    sequence: u64,
) -> Result<Option<Admission>, Error> {
    load(
        staged,
        &admission_key(pid, credential, sequence),
        "admission",
    )
    .await
}

pub async fn mailbox(staged: &StagedStore, pid: &str) -> Result<MailboxUsage, Error> {
    Ok(load(staged, &mailbox_key(pid), "mailbox")
        .await?
        .unwrap_or_default())
}

async fn counter(staged: &StagedStore, key: &[u8], what: &str) -> Result<u64, Error> {
    let Some(bytes) = staged.get(key).await? else {
        return Ok(0);
    };
    let bytes: [u8; 8] = bytes
        .try_into()
        .map_err(|_| Error::Module(format!("invalid {what} counter")))?;
    Ok(u64::from_le_bytes(bytes))
}

pub async fn replay_floor(
    staged: &StagedStore,
    pid: &str,
    credential: Credential,
) -> Result<u64, Error> {
    counter(staged, &replay_floor_key(pid, credential), "replay floor").await
}

pub async fn sender_quota(
    staged: &StagedStore,
    recipient: &str,
    sender: &str,
) -> Result<u64, Error> {
    counter(staged, &sender_quota_key(recipient, sender), "sender quota").await
}

/// [`check_record`] then stage — the single-record writer's shape.
pub fn put<T: Serialize>(
    staged: &mut StagedStore,
    key: Vec<u8>,
    value: &T,
    what: &str,
) -> Result<(), Error> {
    let bytes = sdk::wire::encode(value);
    check_record(&bytes, what)?;
    staged.stage(key, bytes);
    Ok(())
}

pub fn put_counter(staged: &mut StagedStore, key: Vec<u8>, value: u64) {
    staged.stage(key, value.to_le_bytes().to_vec());
}

/// append one committed event and hand back the sequence it took, bumping the
/// conversation's cursor. the CALLER stages the conversation record — every op
/// here checks all of its records before staging any of them, and this one
/// participates in that discipline rather than writing behind the caller's
/// back.
pub fn append_event(
    staged: &mut StagedStore,
    conversation: &mut Conversation,
    now: u64,
    body: crate::interface::EventBody,
) -> Result<u64, Error> {
    let seq = conversation.next_seq;
    conversation.next_seq = seq
        .checked_add(1)
        .ok_or_else(|| Error::Module("conversation event sequence exhausted".into()))?;
    conversation.updated_at = now;
    let event = ConversationEvent { seq, at: now, body };
    put(staged, event_key(&conversation.id, seq), &event, "event")?;
    Ok(seq)
}

/// an encoded [`Message`] must also fit the protocol's own ceiling, which is
/// far below the store's — the spec's 32 KiB total encoded message.
pub fn check_message(message: &Message) -> Result<Vec<u8>, Error> {
    let bytes = sdk::wire::encode(message);
    if bytes.len() > MAX_ENCODED_MESSAGE_BYTES {
        return Err(Error::Module(format!(
            "encoded message is {} bytes, over the {MAX_ENCODED_MESSAGE_BYTES}-byte cap",
            bytes.len()
        )));
    }
    Ok(bytes)
}
