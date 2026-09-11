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
//! | `("b", cid, participant)` | [`Binding`] |
//! | `("h", cid)` | the channel's next event sequence (u64) |
//! | `("e", cid, seq)` | [`ChannelEvent`] — the committed event stream |
//! | `("d", cid, message_seq, recipient)` | [`Delivery`] — the one mutable per-recipient record |
//! | `("q", participant)` | [`MailboxUsage`] |
//! | `("s", recipient, sender)` | that sender's undelivered count in that mailbox |

use sdk::{Error, StagedStore};
use serde::{Serialize, de::DeserializeOwned};

use crate::interface::{Binding, ChannelEvent, Delivery, EventBody, MailboxUsage, Party};

/// write-time cap on ONE stored record, mirroring `tasks::MAX_RECORD_BYTES`:
/// the concrete store's codec bounds a stored value at 1 MiB AT DECODE TIME,
/// so an oversized value would commit fine and then panic every later read on
/// every validator. the 4 KiB margin covers the operation framing.
pub const MAX_RECORD_BYTES: usize = (1 << 20) - 4 * 1024;

fn key(parts: &impl Serialize) -> Vec<u8> {
    sdk::wire::encode(parts)
}

pub fn binding_key(cid: &str, participant: &Party) -> Vec<u8> {
    key(&("b", cid, participant))
}
pub fn head_key(cid: &str) -> Vec<u8> {
    key(&("h", cid))
}
pub fn event_key(cid: &str, seq: u64) -> Vec<u8> {
    key(&("e", cid, seq))
}
pub fn delivery_key(cid: &str, message_seq: u64, recipient: &Party) -> Vec<u8> {
    key(&("d", cid, message_seq, recipient))
}
pub fn mailbox_key(participant: &Party) -> Vec<u8> {
    key(&("q", participant))
}
pub fn sender_quota_key(recipient: &Party, sender: &Party) -> Vec<u8> {
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

pub async fn binding(
    staged: &StagedStore,
    cid: &str,
    participant: &Party,
) -> Result<Option<Binding>, Error> {
    load(staged, &binding_key(cid, participant), "binding").await
}

pub async fn event(
    staged: &StagedStore,
    cid: &str,
    seq: u64,
) -> Result<Option<ChannelEvent>, Error> {
    load(staged, &event_key(cid, seq), "event").await
}

pub async fn delivery(
    staged: &StagedStore,
    cid: &str,
    message_seq: u64,
    recipient: &Party,
) -> Result<Option<Delivery>, Error> {
    load(staged, &delivery_key(cid, message_seq, recipient), "delivery").await
}

pub async fn mailbox(staged: &StagedStore, participant: &Party) -> Result<MailboxUsage, Error> {
    Ok(load(staged, &mailbox_key(participant), "mailbox")
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

/// the channel's next event sequence. sequences are 1-based, so an untouched
/// channel answers 1.
pub async fn head(staged: &StagedStore, cid: &str) -> Result<u64, Error> {
    Ok(counter(staged, &head_key(cid), "event head").await?.max(1))
}

pub async fn sender_quota(
    staged: &StagedStore,
    recipient: &Party,
    sender: &Party,
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
/// channel's head. an event body is bounded by construction (a party, a
/// token, two numbers), so this is the one writer that can stage without a
/// prior check on the caller's side.
pub async fn append_event(
    staged: &mut StagedStore,
    cid: &str,
    now: u64,
    body: EventBody,
) -> Result<u64, Error> {
    let seq = head(staged, cid).await?;
    let next = seq
        .checked_add(1)
        .ok_or_else(|| Error::Module("channel event sequence exhausted".into()))?;
    let event = ChannelEvent { seq, at: now, body };
    put(staged, event_key(cid, seq), &event, "event")?;
    put_counter(staged, head_key(cid), next);
    Ok(seq)
}
