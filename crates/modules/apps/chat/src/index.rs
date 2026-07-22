//! chat's materialized view: full-text message search.
//!
//! canonical chat state serves by-sequence pages and point lookups; it cannot
//! scan or search (any::unordered qmdb — hashed keys). this mapper folds every
//! applied [`ChatMsg`] into a token index and serves `search` as chat's own
//! endpoint on the derived tier.
//!
//! key spaces (inside chat's per-module index database):
//! - `seq/{channel}`                    — mirror of the channel's head_seq.
//!   faithful BY CONSTRUCTION: a failed op aborts its whole block and never
//!   reaches the index, so every applied `PostMessage` assigned exactly the
//!   next sequence, in drain order.
//! - `msg/{channel}/{seq:016x}`         — the current head text of one message
//!   ([`MsgRow`]); edits rewrite it, deletes tombstone it.
//! - `tok/{token}/{channel}/{seq:016x}` — one posting per (token, message),
//!   value = [`TokRef`]. postings of a tombstoned/retokenized head are
//!   deleted in the same fold, so search never surfaces stale text.
//! - `tag/{label}/{channel}/{rseq}` and `tagcat/{channel}/{label}` — the
//!   hashtag postings and per-channel tag catalog (see [`tags`]), maintained
//!   by the same fold with the same tombstone/re-fold discipline.
//!
//! caveat (shared by every mapper): the view reflects ops folded SINCE the
//! index existed. enabling an index against storage with prior chat history
//! leaves that history unsearchable — and the seq mirror offset — until the
//! chain is replayed through the feed.
//!
//! this file is the DECISION core — pure functions over [`StateRead`],
//! compiled natively and unit-tested against a plain map. the wasm shell
//! (`src/index_guest.rs`, feature `index-guest`) wires it into the engine.
//! within one op a read never sees that op's own writes (they apply after
//! the decision); across ops in one feed batch it sees everything earlier —
//! identical in the engine transaction and the native test harness.

use index_guest::search::{self, DEFAULT_POSTING_CAP};
use index_guest::{Fail, OpRow, OriginKind, OriginTag, StateRead, Writes};
use serde::{Deserialize, Serialize};

use crate::{Block, ChatMsg, Span, decode_msg};

mod tags;
pub use tags::{MAX_TAG_CHARS, MAX_TAGS_PER_MESSAGE, TagRow};

/// default and max page size for search results.
const DEFAULT_SEARCH_LIMIT: usize = 20;
const MAX_SEARCH_LIMIT: usize = 100;

/// [`Fail`] code: an applied op's payload did not decode — the interface
/// crates drifted, which only a refold can honestly repair. fail loudly (the
/// feed holds), never guess.
const FAIL_OP_DECODE: i32 = 2;
/// [`Fail`] code: a stored row did not decode — a damaged read model.
const FAIL_ROW_DECODE: i32 = 3;
/// [`Fail`] code: a view request this mapper does not speak.
const FAIL_BAD_REQUEST: i32 = 4;

/// the stored head row of one message, as search results return it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MsgRow {
    pub channel_id: String,
    pub seq: u64,
    pub message_id: String,
    /// rendered author: `user:{id}`, `agent:{module}/{agent}`, `module:{id}`,
    /// or `system` — display-grade, derived from the dispatch origin exactly
    /// like chat derives authorship.
    pub author: String,
    pub height: u64,
    pub time: u64,
    pub text: String,
    pub deleted: bool,
    pub edited: bool,
    pub thread: Option<u64>,
    /// the head's normalized tag labels (appearance order, ≤ 16) — stored so
    /// an edit/delete can diff/clear exactly what this head indexed (the flat
    /// `text` can't re-derive them: it folds code blocks in). tombstones
    /// carry none, like their empty text.
    pub tags: Vec<String>,
}

/// a token posting's value: enough to rank (time) and fetch the row.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct TokRef {
    channel_id: String,
    seq: u64,
    message_id: String,
    time: u64,
}

/// chat's view requests. externally tagged json, matching the module wire
/// style: `{"search": {"text": "...", "channel_id": "...", "limit": 20}}`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatViewQuery {
    Search {
        text: String,
        #[serde(default)]
        channel_id: Option<String>,
        #[serde(default)]
        limit: Option<usize>,
    },
    /// the tag catalog: `{"tags": {"channel_id": "...", "limit": 20}}`. no
    /// channel aggregates every channel per label.
    Tags {
        #[serde(default)]
        channel_id: Option<String>,
        #[serde(default)]
        limit: Option<usize>,
    },
    /// live messages carrying one exact tag, newest first:
    /// `{"tag_search": {"tag": "rust", "channel_id": "...", "limit": 20}}`.
    TagSearch {
        tag: String,
        #[serde(default)]
        channel_id: Option<String>,
        #[serde(default)]
        limit: Option<usize>,
    },
}

/// chat's view replies: `{"hits": [<MsgRow>…]}` newest first, or
/// `{"tags": [<TagRow>…]}` count-ordered.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatViewReply {
    Hits(Vec<MsgRow>),
    Tags(Vec<TagRow>),
}

fn seq_key(channel: &str) -> String {
    format!("seq/{channel}")
}

fn msg_key(channel: &str, seq: u64) -> String {
    format!("msg/{channel}/{seq:016x}")
}

fn tok_key(token: &str, channel: &str, seq: u64) -> String {
    format!("tok/{token}/{channel}/{seq:016x}")
}

/// flatten message blocks to the plain text the token index sees.
fn plain_text(blocks: &[Block]) -> String {
    fn spans(out: &mut String, spans: &[Span]) {
        for span in spans {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
            out.push_str(&span.text);
        }
    }
    let mut out = String::new();
    for block in blocks {
        match block {
            Block::Paragraph(s) | Block::Quote(s) => spans(&mut out, s),
            Block::Code { text, .. } => {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(text);
            }
            Block::Divider => {}
        }
    }
    out
}

/// render the author the way chat derives it: origin decides, `as_agent`
/// refines a module origin into an agent author.
fn author(origin: &OriginTag, as_agent: Option<&str>) -> String {
    let id = origin.id.as_deref().unwrap_or_default();
    match (origin.kind, as_agent) {
        (OriginKind::Module, Some(agent)) => format!("agent:{id}/{agent}"),
        (OriginKind::Module, None) => format!("module:{id}"),
        (OriginKind::External, _) => format!("user:{id}"),
        (OriginKind::System, _) => "system".to_string(),
    }
}

fn read_u64(read: &impl StateRead, key: &str) -> u64 {
    read.get(key.as_bytes())
        .and_then(|v| <[u8; 8]>::try_from(v.as_slice()).ok())
        .map(u64::from_be_bytes)
        .unwrap_or(0)
}

fn read_row(read: &impl StateRead, key: &str) -> Result<Option<MsgRow>, Fail> {
    let Some(bytes) = read.get(key.as_bytes()) else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))
}

/// stage every entry one head state materializes to — the row plus one
/// posting per token and per tag label (a tombstone's empty text and tag set
/// yield none) — so every write path produces byte-identical rows.
fn put_row_and_toks(out: &mut Writes, row: &MsgRow) -> Result<(), Fail> {
    index_guest::put(
        out,
        msg_key(&row.channel_id, row.seq),
        serde_json::to_vec(row).map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))?,
    );
    let tok_ref = serde_json::to_vec(&TokRef {
        channel_id: row.channel_id.clone(),
        seq: row.seq,
        message_id: row.message_id.clone(),
        time: row.time,
    })
    .map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))?;
    for token in search::tokens(&row.text) {
        index_guest::put(
            out,
            tok_key(&token, &row.channel_id, row.seq),
            tok_ref.clone(),
        );
    }
    for label in &row.tags {
        index_guest::put(
            out,
            tags::tag_key(label, &row.channel_id, row.seq),
            tok_ref.clone(),
        );
    }
    Ok(())
}

fn delete_toks(out: &mut Writes, row: &MsgRow) {
    for token in search::tokens(&row.text) {
        index_guest::delete(out, tok_key(&token, &row.channel_id, row.seq));
    }
}

/// fold one applied op into derived writes. an applied op decoded fine in the
/// module; failing HERE means the interface crates drifted — fail loudly (the
/// feed holds and surfaces it), never guess.
pub fn fold_op(op: &OpRow, read: &impl StateRead) -> Result<Writes, Fail> {
    let msg = decode_msg(&op.payload).map_err(|e| Fail::new(FAIL_OP_DECODE, e))?;
    let mut out = Writes::new();
    match msg {
        ChatMsg::PostMessage {
            channel_id,
            message_id,
            blocks,
            thread,
            as_agent,
        } => {
            let seq = read_u64(read, &seq_key(&channel_id)) + 1;
            index_guest::put(&mut out, seq_key(&channel_id), seq.to_be_bytes().to_vec());
            let row = MsgRow {
                seq,
                message_id,
                author: author(&op.origin, as_agent.as_deref()),
                height: op.height,
                time: op.time,
                text: plain_text(&blocks),
                deleted: false,
                edited: false,
                thread,
                tags: tags::labels(&blocks),
                channel_id,
            };
            tags::fold_catalog(read, &mut out, &row.channel_id, &[], &row.tags)?;
            put_row_and_toks(&mut out, &row)?;
        }
        ChatMsg::EditMessage {
            channel_id,
            seq,
            blocks,
            ..
        } => {
            // absent row == the message predates this index; nothing to
            // retokenize (see the module doc's pre-index caveat).
            let Some(mut row) = read_row(read, &msg_key(&channel_id, seq))? else {
                return Ok(out);
            };
            if row.deleted {
                return Ok(out);
            }
            // delete BEFORE re-putting: tokens/tags shared by the old and
            // new text stage a delete then a put, and the last command
            // wins. the catalog moves by the old/new tag-set DIFF.
            delete_toks(&mut out, &row);
            tags::delete_postings(&mut out, &row);
            let new_tags = tags::labels(&blocks);
            tags::fold_catalog(read, &mut out, &row.channel_id, &row.tags, &new_tags)?;
            row.text = plain_text(&blocks);
            row.tags = new_tags;
            row.edited = true;
            put_row_and_toks(&mut out, &row)?;
        }
        ChatMsg::DeleteMessage { channel_id, seq } => {
            let Some(mut row) = read_row(read, &msg_key(&channel_id, seq))? else {
                return Ok(out);
            };
            delete_toks(&mut out, &row);
            tags::delete_postings(&mut out, &row);
            tags::fold_catalog(read, &mut out, &row.channel_id, &row.tags, &[])?;
            row.deleted = true;
            row.text = String::new();
            row.tags = Vec::new();
            put_row_and_toks(&mut out, &row)?;
        }
        // channel records (create/rename/archive), reactions, hooks,
        // membership, and huddle rosters don't change any searchable text —
        // no view impact.
        ChatMsg::CreateChannel { .. }
        | ChatMsg::RenameChannel { .. }
        | ChatMsg::SetChannelArchived { .. }
        | ChatMsg::AddReaction { .. }
        | ChatMsg::RemoveReaction { .. }
        | ChatMsg::RegisterHook { .. }
        | ChatMsg::UnregisterHook { .. }
        | ChatMsg::SetMembership { .. }
        | ChatMsg::JoinHuddle { .. }
        | ChatMsg::LeaveHuddle { .. }
        | ChatMsg::SweepHuddle { .. } => {}
    }
    Ok(out)
}

/// serve one materialized-view request.
pub fn serve_view(read: &impl StateRead, req: &[u8]) -> Result<Vec<u8>, Fail> {
    let query: ChatViewQuery =
        serde_json::from_slice(req).map_err(|e| Fail::new(FAIL_BAD_REQUEST, e.to_string()))?;
    match query {
        ChatViewQuery::Search {
            text,
            channel_id,
            limit,
        } => {
            let tokens: Vec<String> = search::tokens(&text).into_iter().collect();
            if tokens.is_empty() {
                return Err(Fail::new(FAIL_BAD_REQUEST, "search text has no tokens"));
            }
            // each token matches as a prefix; the channel scope filters the
            // intersected refs by their stored channel (postings can't embed
            // it after a partial token).
            let mut refs: Vec<TokRef> =
                search::intersect_prefix(read, "tok/", &tokens, DEFAULT_POSTING_CAP)
                    .into_iter()
                    .filter_map(|hit| serde_json::from_slice(&hit.value).ok())
                    .filter(|r: &TokRef| channel_id.as_ref().is_none_or(|c| &r.channel_id == c))
                    .collect();
            // newest first; (channel, seq) tiebreak for a stable order.
            refs.sort_by(|a, b| {
                (b.time, &b.channel_id, b.seq).cmp(&(a.time, &a.channel_id, a.seq))
            });
            let limit = limit
                .unwrap_or(DEFAULT_SEARCH_LIMIT)
                .clamp(1, MAX_SEARCH_LIMIT);
            let mut hits = Vec::new();
            for r in refs.into_iter().take(limit) {
                if let Some(bytes) = read.get(msg_key(&r.channel_id, r.seq).as_bytes()) {
                    let row: MsgRow = serde_json::from_slice(&bytes)
                        .map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))?;
                    hits.push(row);
                }
            }
            serde_json::to_vec(&ChatViewReply::Hits(hits))
                .map_err(|e| Fail::new(FAIL_BAD_REQUEST, e.to_string()))
        }
        ChatViewQuery::Tags { channel_id, limit } => {
            let rows = tags::serve_tags(read, channel_id, limit)?;
            serde_json::to_vec(&ChatViewReply::Tags(rows))
                .map_err(|e| Fail::new(FAIL_BAD_REQUEST, e.to_string()))
        }
        ChatViewQuery::TagSearch {
            tag,
            channel_id,
            limit,
        } => {
            let hits = tags::serve_tag_search(read, &tag, channel_id, limit)?;
            serde_json::to_vec(&ChatViewReply::Hits(hits))
                .map_err(|e| Fail::new(FAIL_BAD_REQUEST, e.to_string()))
        }
    }
}

#[cfg(test)]
mod tag_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode_msg;
    use index_guest::apply_to_map;
    use std::collections::BTreeMap;

    type Map = BTreeMap<Vec<u8>, Vec<u8>>;

    fn op(height: u64, msg: &ChatMsg) -> OpRow {
        OpRow {
            height,
            seq: 0,
            time: 1_000 + height,
            origin: OriginTag::external("jess"),
            payload: encode_msg(msg),
        }
    }

    fn post(channel: &str, id: &str, text: &str) -> ChatMsg {
        ChatMsg::PostMessage {
            channel_id: channel.into(),
            message_id: id.into(),
            blocks: vec![Block::paragraph(text)],
            thread: None,
            as_agent: None,
        }
    }

    fn fold(map: &mut Map, height: u64, msg: &ChatMsg) {
        let writes = fold_op(&op(height, msg), map).expect("fold");
        apply_to_map(map, writes);
    }

    fn search(map: &Map, req: serde_json::Value) -> Vec<MsgRow> {
        let bytes = serve_view(map, &serde_json::to_vec(&req).unwrap()).expect("view");
        match serde_json::from_slice(&bytes).expect("reply decodes") {
            ChatViewReply::Hits(hits) => hits,
            other => panic!("expected hits, got {other:?}"),
        }
    }

    #[test]
    fn posts_are_searchable_and_seq_mirrors() {
        let mut map = Map::new();
        fold(&mut map, 1, &post("general", "m1", "hello fluent world"));
        fold(&mut map, 2, &post("general", "m2", "unrelated words"));

        let hits = search(&map, serde_json::json!({"search": {"text": "fluent"}}));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].message_id, "m1");
        assert_eq!(hits[0].seq, 1);
        assert_eq!(hits[0].author, "user:jess");
        assert_eq!(
            map.get(b"seq/general".as_slice()),
            Some(&2u64.to_be_bytes().to_vec()),
            "the seq mirror tracks the channel head"
        );
    }

    #[test]
    fn edits_retokenize_and_deletes_tombstone() {
        let mut map = Map::new();
        fold(&mut map, 1, &post("g", "m1", "original wording"));
        fold(
            &mut map,
            2,
            &ChatMsg::EditMessage {
                channel_id: "g".into(),
                seq: 1,
                blocks: vec![Block::paragraph("revised phrasing")],
                base_rev: None,
            },
        );

        assert!(search(&map, serde_json::json!({"search": {"text": "original"}})).is_empty());
        let hits = search(&map, serde_json::json!({"search": {"text": "revised"}}));
        assert_eq!(hits.len(), 1);
        assert!(hits[0].edited);

        fold(
            &mut map,
            3,
            &ChatMsg::DeleteMessage {
                channel_id: "g".into(),
                seq: 1,
            },
        );
        assert!(search(&map, serde_json::json!({"search": {"text": "revised"}})).is_empty());
        let row: MsgRow =
            serde_json::from_slice(map.get(msg_key("g", 1).as_bytes()).expect("tombstone"))
                .unwrap();
        assert!(row.deleted);
        assert!(row.text.is_empty());
    }

    #[test]
    fn channel_scope_and_multi_token_intersection() {
        let mut map = Map::new();
        fold(&mut map, 1, &post("general", "m1", "deploy pipeline green"));
        fold(&mut map, 2, &post("random", "m2", "deploy thoughts"));

        // multi-token AND: only m1 carries both.
        let hits = search(
            &map,
            serde_json::json!({"search": {"text": "deploy green"}}),
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].message_id, "m1");

        // channel scope filters by the stored ref's channel.
        let hits = search(
            &map,
            serde_json::json!({"search": {"text": "deploy", "channel_id": "random"}}),
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].message_id, "m2");
    }

    #[test]
    fn prefix_match_and_ranking_newest_first() {
        let mut map = Map::new();
        fold(&mut map, 1, &post("g", "m1", "testing the indexer"));
        fold(&mut map, 2, &post("g", "m2", "tested and shipped"));

        // `test` prefix-matches both `testing` and `tested`; newest first.
        let hits = search(&map, serde_json::json!({"search": {"text": "test"}}));
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].message_id, "m2");
    }

    #[test]
    fn prefix_match_dedups_multiple_words_per_message() {
        let mut map = Map::new();
        fold(&mut map, 1, &post("g", "m1", "test tested testing"));
        let hits = search(&map, serde_json::json!({"search": {"text": "tes"}}));
        assert_eq!(hits.len(), 1, "three matching words collapse to one hit");
        assert_eq!(hits[0].message_id, "m1");
    }

    #[test]
    fn agent_author_is_rendered() {
        let mut map = Map::new();
        let writes = fold_op(
            &OpRow {
                height: 1,
                seq: 0,
                time: 1_001,
                origin: OriginTag::module("agent"),
                payload: encode_msg(&ChatMsg::PostMessage {
                    channel_id: "g".into(),
                    message_id: "m1".into(),
                    blocks: vec![Block::paragraph("from the helper")],
                    thread: None,
                    as_agent: Some("helper".into()),
                }),
            },
            &map,
        )
        .expect("fold");
        apply_to_map(&mut map, writes);

        let hits = search(&map, serde_json::json!({"search": {"text": "helper"}}));
        assert_eq!(hits[0].author, "agent:agent/helper");
    }

    #[test]
    fn bad_view_requests_fail_cleanly() {
        let map = Map::new();
        assert!(serve_view(&map, b"not json").is_err());
        assert!(
            serve_view(&map, br#"{"search": {"text": "!!!"}}"#).is_err(),
            "no tokens is a view error"
        );
    }
}
