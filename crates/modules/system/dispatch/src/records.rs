//! the dispatch module's STORE key space and per-record codecs.
//!
//! every logical record is its own store key, so an op touches only the keys it
//! names and `root()` is the store's cached merkle root — never a
//! re-serialization of the whole plane.
//!
//! | logical key | value |
//! |---|---|
//! | `r/{recipe_id}` | one [`Recipe`] record |
//! | `r#` | the recipe-id enumeration index (a json `BTreeSet<String>`) |
//! | `d/{receiver}\x1f{dispatch_id}` | one [`DispatchState`] record |
//! | `q/{seq}` | one mailbox entry: the dispatch key it delivers |
//! | `q#` | the mailbox cursor `head‖next` (two u64 LE) |
//!
//! the store hashes its keys and cannot enumerate, so anything a reader walks
//! needs an index of its own:
//!
//! * [`DispatchQuery::Recipes`] is an unpaged read of every recipe, so the ids
//!   live in the `r#` index record.
//! * `DeliverPending` sweeps the mailbox in FIFO order. entries are only ever
//!   APPENDED at `next` and REMOVED from the front, so the queue is the
//!   contiguous range `head..next` and the cursor record is all the index it
//!   needs — a fixed 16 bytes no traffic can grow.
//!
//! an empty collection DROPS its key (`r#` with no recipes, `q#` with a drained
//! mailbox), so a plane whose mailbox emptied hashes exactly like one that never
//! enqueued anything.
//!
//! the record codec is the length-prefixed [`sdk::codec`] toolkit, NOT json: a
//! dispatch record carries the raw outcome bytes (up to [`crate::MAX_RESULT_BYTES`],
//! 256 KiB) and serde_json renders a `Vec<u8>` as an array of decimal numbers —
//! up to 4 bytes per byte, which would push a legitimate 256 KiB result over the
//! store's 1 MiB record cap. the recipe record rides the same codec so a
//! `Routing::Pinned` key is stored verbatim too.

use std::collections::BTreeSet;

use saga::{put_origin, take_origin};
use sdk::{Error, StagedStore, codec};

use crate::{DispatchState, OutputContract, Recipe, Routing, Status};

/// one recipe record per id.
const RECIPE_PREFIX: &[u8] = b"r/";
/// the recipe-id enumeration index — what [`crate::DispatchQuery::Recipes`]
/// walks.
///
// ponytail: ONE index record holds every recipe id, so a registration is
// O(all ids) in bytes and the plane stops registering when that record hits
// MAX_RECORD_BYTES (~8k ids at the MAX_ID_BYTES cap). unlike tasks' `t#`, this
// ceiling is NOT a one-way door: `RemoveRecipe` frees an owner's bytes back.
// shard the index by id prefix before a network is expected to carry that many
// recipes.
const RECIPE_INDEX_KEY: &[u8] = b"r#";
/// one dispatch record per composite (receiver, dispatch_id) key.
const DISPATCH_PREFIX: &[u8] = b"d/";
/// one mailbox entry per delivery seq.
const MAILBOX_PREFIX: &[u8] = b"q/";
/// the mailbox cursor: `head‖next`, two u64 LE. absent = the empty queue.
const MAILBOX_CURSOR_KEY: &[u8] = b"q#";

fn prefixed(prefix: &[u8], rest: &[u8]) -> Vec<u8> {
    let mut key = prefix.to_vec();
    key.extend_from_slice(rest);
    key
}

pub(crate) fn recipe_key(recipe_id: &str) -> Vec<u8> {
    prefixed(RECIPE_PREFIX, recipe_id.as_bytes())
}

pub(crate) fn dispatch_key_of(key: &str) -> Vec<u8> {
    prefixed(DISPATCH_PREFIX, key.as_bytes())
}

pub(crate) fn mailbox_key(seq: u64) -> Vec<u8> {
    prefixed(MAILBOX_PREFIX, &seq.to_le_bytes())
}

pub(crate) fn recipe_index_key() -> Vec<u8> {
    RECIPE_INDEX_KEY.to_vec()
}

// ---- record codecs ---------------------------------------------------------

fn contract_tag(contract: OutputContract) -> u8 {
    match contract {
        OutputContract::Text => 0,
        OutputContract::Json => 1,
    }
}

fn take_contract(cur: &mut codec::Cursor) -> Result<OutputContract, Error> {
    match cur.byte("record contract")? {
        0 => Ok(OutputContract::Text),
        1 => Ok(OutputContract::Json),
        d => Err(Error::Module(format!(
            "record has unknown contract discriminant {d}"
        ))),
    }
}

pub(crate) fn encode_recipe(r: &Recipe) -> Vec<u8> {
    let mut out = Vec::new();
    codec::push_str(&mut out, &r.recipe_id);
    put_origin(&mut out, &r.owner);
    codec::push_str(&mut out, &r.description);
    codec::push_str(&mut out, &r.capability);
    match &r.routing {
        Routing::Rendezvous => out.push(0),
        Routing::Pinned(key) => {
            out.push(1);
            codec::push_bytes(&mut out, key);
        }
    }
    out.push(contract_tag(r.output_contract));
    out.extend_from_slice(&r.max_attempts.to_le_bytes());
    codec::push_opt_u64(&mut out, r.deadline_views);
    codec::push_opt_u64(&mut out, r.lease_views);
    out.extend_from_slice(&r.created_at.to_le_bytes());
    out.extend_from_slice(&r.updated_at.to_le_bytes());
    out
}

pub(crate) fn decode_recipe(bytes: &[u8]) -> Result<Recipe, Error> {
    let mut cur = codec::Cursor::new(bytes);
    let recipe_id = cur.string("recipe id")?;
    let owner = take_origin(&mut cur)?;
    let description = cur.string("recipe description")?;
    let capability = cur.string("recipe capability")?;
    let routing = match cur.byte("recipe routing")? {
        0 => Routing::Rendezvous,
        1 => Routing::Pinned(cur.bytes("recipe routing pin")?.to_vec()),
        d => {
            return Err(Error::Module(format!(
                "record has unknown routing discriminant {d}"
            )));
        }
    };
    let output_contract = take_contract(&mut cur)?;
    let max_attempts = cur.u32("recipe max_attempts")?;
    let deadline_views = cur.opt_u64("recipe deadline_views")?;
    let lease_views = cur.opt_u64("recipe lease_views")?;
    let created_at = cur.u64("recipe created_at")?;
    let updated_at = cur.u64("recipe updated_at")?;
    cur.finish("recipe record")?;
    Ok(Recipe {
        recipe_id,
        owner,
        description,
        capability,
        routing,
        output_contract,
        max_attempts,
        deadline_views,
        lease_views,
        created_at,
        updated_at,
    })
}

pub(crate) fn encode_dispatch(d: &DispatchState) -> Vec<u8> {
    let mut out = Vec::new();
    codec::push_str(&mut out, &d.receiver);
    codec::push_str(&mut out, &d.dispatch_id);
    codec::push_str(&mut out, &d.recipe_id);
    out.push(contract_tag(d.contract));
    codec::push_str(&mut out, &d.saga_id);
    out.push(match d.status {
        Status::AwaitingResult => 0,
        Status::AwaitingDelivery => 1,
        Status::Delivered => 2,
    });
    match &d.outcome {
        None => out.push(0),
        Some(Ok(bytes)) => {
            out.push(1);
            codec::push_bytes(&mut out, bytes);
        }
        Some(Err(error)) => {
            out.push(2);
            codec::push_str(&mut out, error);
        }
    }
    out.extend_from_slice(&d.created_at.to_le_bytes());
    out.extend_from_slice(&d.updated_at.to_le_bytes());
    out
}

pub(crate) fn decode_dispatch(bytes: &[u8]) -> Result<DispatchState, Error> {
    let mut cur = codec::Cursor::new(bytes);
    let receiver = cur.string("dispatch receiver")?;
    let dispatch_id = cur.string("dispatch id")?;
    let recipe_id = cur.string("dispatch recipe id")?;
    let contract = take_contract(&mut cur)?;
    let saga_id = cur.string("dispatch saga id")?;
    let status = match cur.byte("dispatch status")? {
        0 => Status::AwaitingResult,
        1 => Status::AwaitingDelivery,
        2 => Status::Delivered,
        d => {
            return Err(Error::Module(format!(
                "record has unknown status discriminant {d}"
            )));
        }
    };
    let outcome = match cur.byte("dispatch outcome tag")? {
        0 => None,
        1 => Some(Ok(cur.bytes("dispatch outcome")?.to_vec())),
        2 => Some(Err(cur.string("dispatch outcome error")?)),
        t => {
            return Err(Error::Module(format!("record has unknown outcome tag {t}")));
        }
    };
    let created_at = cur.u64("dispatch created_at")?;
    let updated_at = cur.u64("dispatch updated_at")?;
    cur.finish("dispatch record")?;
    Ok(DispatchState {
        receiver,
        dispatch_id,
        recipe_id,
        contract,
        saga_id,
        status,
        outcome,
        created_at,
        updated_at,
    })
}

// ---- the mailbox cursor ----------------------------------------------------

/// the FIFO's bounds. entries occupy exactly `head..next`: `on_saga_callback`
/// appends at `next`, `on_deliver_pending` removes a PREFIX, so the range is
/// contiguous by construction and the queue length is `next - head`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Mailbox {
    pub(crate) head: u64,
    pub(crate) next: u64,
}

impl Mailbox {
    pub(crate) fn len(&self) -> u64 {
        self.next.saturating_sub(self.head)
    }
}

fn decode_mailbox(bytes: &[u8]) -> Result<Mailbox, Error> {
    let mut cur = codec::Cursor::new(bytes);
    let head = cur.u64("mailbox head")?;
    let next = cur.u64("mailbox next")?;
    cur.finish("mailbox cursor")?;
    Ok(Mailbox { head, next })
}

/// stage the cursor. a DRAINED mailbox drops the key entirely, so a plane whose
/// deliveries all landed hashes exactly like one that never enqueued — and the
/// seq numbering restarts from 0 with no live entry to collide with.
pub(crate) fn stage_mailbox(staged: &mut StagedStore, mailbox: Mailbox) {
    if mailbox.len() == 0 {
        staged.delete(MAILBOX_CURSOR_KEY.to_vec());
        return;
    }
    let mut out = Vec::with_capacity(16);
    out.extend_from_slice(&mailbox.head.to_le_bytes());
    out.extend_from_slice(&mailbox.next.to_le_bytes());
    staged.stage(MAILBOX_CURSOR_KEY.to_vec(), out);
}

// ---- reads -----------------------------------------------------------------
//
// EXECUTE reads go through the staged overlay (a transition must see an earlier
// same-block write); QUERY reads are COMMITTED-ONLY — the host's delivery
// injection and runs' turn-taken check decide over the frozen end-of-block
// state, so a staged write must never leak into them. the two are separate
// named functions on purpose: neither caller can pick the wrong view by
// flipping an argument.

fn decoded<T>(
    raw: Option<Vec<u8>>,
    decode: fn(&[u8]) -> Result<T, Error>,
) -> Result<Option<T>, Error> {
    raw.as_deref().map(decode).transpose()
}

pub(crate) async fn staged_recipe(
    staged: &StagedStore,
    recipe_id: &str,
) -> Result<Option<Recipe>, Error> {
    decoded(staged.get(&recipe_key(recipe_id)).await?, decode_recipe)
}

pub(crate) async fn committed_recipe(
    staged: &StagedStore,
    recipe_id: &str,
) -> Result<Option<Recipe>, Error> {
    decoded(
        staged.get_committed(&recipe_key(recipe_id)).await?,
        decode_recipe,
    )
}

pub(crate) async fn staged_dispatch(
    staged: &StagedStore,
    key: &str,
) -> Result<Option<DispatchState>, Error> {
    decoded(staged.get(&dispatch_key_of(key)).await?, decode_dispatch)
}

pub(crate) async fn committed_dispatch(
    staged: &StagedStore,
    key: &str,
) -> Result<Option<DispatchState>, Error> {
    decoded(
        staged.get_committed(&dispatch_key_of(key)).await?,
        decode_dispatch,
    )
}

/// the recipe-id index through the staged overlay. absent reads as the empty
/// set; `BTreeSet` serializes ASCENDING, so the record bytes are canonical and
/// `Recipes` answers in the same order the old `BTreeMap` walk did.
pub(crate) async fn staged_recipe_index(staged: &StagedStore) -> Result<BTreeSet<String>, Error> {
    decode_recipe_index(staged.get(RECIPE_INDEX_KEY).await?)
}

pub(crate) async fn committed_recipe_index(
    staged: &StagedStore,
) -> Result<BTreeSet<String>, Error> {
    decode_recipe_index(staged.get_committed(RECIPE_INDEX_KEY).await?)
}

fn decode_recipe_index(raw: Option<Vec<u8>>) -> Result<BTreeSet<String>, Error> {
    let Some(bytes) = raw else {
        return Ok(BTreeSet::new());
    };
    sdk::wire::decode(&bytes).map_err(|e| Error::Module(format!("recipe index decode: {e}")))
}

pub(crate) fn encode_recipe_index(ids: &BTreeSet<String>) -> Vec<u8> {
    sdk::wire::encode(ids)
}

pub(crate) async fn staged_mailbox(staged: &StagedStore) -> Result<Mailbox, Error> {
    match staged.get(MAILBOX_CURSOR_KEY).await? {
        Some(bytes) => decode_mailbox(&bytes),
        None => Ok(Mailbox::default()),
    }
}

pub(crate) async fn committed_mailbox(staged: &StagedStore) -> Result<Mailbox, Error> {
    match staged.get_committed(MAILBOX_CURSOR_KEY).await? {
        Some(bytes) => decode_mailbox(&bytes),
        None => Ok(Mailbox::default()),
    }
}
