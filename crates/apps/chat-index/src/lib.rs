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
//!
//! caveat (shared by every mapper): the view reflects ops applied SINCE the
//! index existed. enabling an index against storage with prior chat history
//! leaves that history unsearchable — and the seq mirror offset — until the
//! chain (or the index together with the local block counter feeding it) is
//! rebuilt from genesis.

use chat_interface::{Block, ChatMsg, DEFAULT_CHAT_TARGET, Span, decode_msg};
use indexer::search::{self, DEFAULT_POSTING_CAP};
use indexer::{
    ApplyCtx, Derived, Error, ModuleIndexer, OpMeta, OriginKind, OriginTag, Result, ViewReader,
};
use serde::{Deserialize, Serialize};

/// default and max page size for search results.
const DEFAULT_SEARCH_LIMIT: usize = 20;
const MAX_SEARCH_LIMIT: usize = 100;

/// the stored head row of one message, as search results return it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread: Option<u64>,
}

/// a token posting's value: enough to rank (time) and fetch the row.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TokRef {
    channel_id: String,
    seq: u64,
    message_id: String,
    time: u64,
}

/// chat's view requests. externally tagged json, matching the module wire
/// style: `{"search": {"text": "...", "channelId": "...", "limit": 20}}`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChatViewQuery {
    #[serde(rename_all = "camelCase")]
    Search {
        text: String,
        #[serde(default)]
        channel_id: Option<String>,
        #[serde(default)]
        limit: Option<usize>,
    },
}

/// chat's view replies: `{"hits": [<MsgRow>…]}`, newest first.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChatViewReply {
    Hits(Vec<MsgRow>),
}

/// the chat mapper. register with the module's genesis id.
pub struct ChatIndex {
    module: String,
}

impl ChatIndex {
    pub fn new(module: impl Into<String>) -> Self {
        Self {
            module: module.into(),
        }
    }
}

impl Default for ChatIndex {
    fn default() -> Self {
        Self::new(DEFAULT_CHAT_TARGET)
    }
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

fn read_u64(ctx: &ApplyCtx, key: &str) -> Result<u64> {
    Ok(ctx
        .get(key.as_bytes())?
        .and_then(|v| <[u8; 8]>::try_from(v.as_slice()).ok())
        .map(u64::from_be_bytes)
        .unwrap_or(0))
}

fn read_row(ctx: &ApplyCtx, key: &str) -> Result<Option<MsgRow>> {
    match ctx.get(key.as_bytes())? {
        Some(bytes) => Ok(Some(
            serde_json::from_slice(&bytes).map_err(|e| Error::Mapper(e.to_string()))?,
        )),
        None => Ok(None),
    }
}

fn put_row(out: &mut Derived, row: &MsgRow) -> Result<()> {
    out.put(
        msg_key(&row.channel_id, row.seq),
        serde_json::to_vec(row).map_err(|e| Error::Mapper(e.to_string()))?,
    );
    Ok(())
}

/// write (or delete) the postings of one head state.
fn put_toks(out: &mut Derived, row: &MsgRow) -> Result<()> {
    let tok_ref = serde_json::to_vec(&TokRef {
        channel_id: row.channel_id.clone(),
        seq: row.seq,
        message_id: row.message_id.clone(),
        time: row.time,
    })
    .map_err(|e| Error::Mapper(e.to_string()))?;
    for token in search::tokens(&row.text) {
        out.put(tok_key(&token, &row.channel_id, row.seq), tok_ref.clone());
    }
    Ok(())
}

fn delete_toks(out: &mut Derived, row: &MsgRow) {
    for token in search::tokens(&row.text) {
        out.delete(tok_key(&token, &row.channel_id, row.seq));
    }
}

impl ModuleIndexer for ChatIndex {
    fn module(&self) -> &str {
        &self.module
    }

    fn index_op(
        &self,
        ctx: &ApplyCtx,
        meta: &OpMeta,
        payload: &[u8],
        out: &mut Derived,
    ) -> Result<()> {
        // an applied op decoded fine in the module; failing HERE means the
        // interface crates drifted — poison loudly, never guess.
        let msg = decode_msg(payload).map_err(Error::Mapper)?;
        match msg {
            ChatMsg::PostMessage {
                channel_id,
                message_id,
                blocks,
                thread,
                as_agent,
            } => {
                let seq = read_u64(ctx, &seq_key(&channel_id))? + 1;
                out.put(seq_key(&channel_id), seq.to_be_bytes().to_vec());
                let row = MsgRow {
                    seq,
                    message_id,
                    author: author(meta.origin, as_agent.as_deref()),
                    height: meta.height,
                    time: meta.time,
                    text: plain_text(&blocks),
                    deleted: false,
                    edited: false,
                    thread,
                    channel_id,
                };
                put_toks(out, &row)?;
                put_row(out, &row)
            }
            ChatMsg::EditMessage {
                channel_id, seq, blocks, ..
            } => {
                // absent row == the message predates this index; nothing to
                // retokenize (see the module doc's pre-index caveat).
                let Some(mut row) = read_row(ctx, &msg_key(&channel_id, seq))? else {
                    return Ok(());
                };
                if row.deleted {
                    return Ok(());
                }
                delete_toks(out, &row);
                row.text = plain_text(&blocks);
                row.edited = true;
                put_toks(out, &row)?;
                put_row(out, &row)
            }
            ChatMsg::DeleteMessage { channel_id, seq } => {
                let Some(mut row) = read_row(ctx, &msg_key(&channel_id, seq))? else {
                    return Ok(());
                };
                delete_toks(out, &row);
                row.deleted = true;
                row.text = String::new();
                put_row(out, &row)
            }
            // channel records, reactions, hooks, and membership don't change
            // any searchable text — no view impact.
            ChatMsg::CreateChannel { .. }
            | ChatMsg::AddReaction { .. }
            | ChatMsg::RemoveReaction { .. }
            | ChatMsg::RegisterHook { .. }
            | ChatMsg::UnregisterHook { .. }
            | ChatMsg::SetMembership { .. } => Ok(()),
        }
    }

    fn serve_view(&self, reader: &ViewReader, req: &[u8]) -> Result<Vec<u8>> {
        let query: ChatViewQuery =
            serde_json::from_slice(req).map_err(|e| Error::View(e.to_string()))?;
        match query {
            ChatViewQuery::Search {
                text,
                channel_id,
                limit,
            } => {
                let tokens = search::tokens(&text);
                if tokens.is_empty() {
                    return Err(Error::View("search text has no tokens".into()));
                }
                let prefixes: Vec<String> = tokens
                    .iter()
                    .map(|t| match &channel_id {
                        Some(c) => format!("tok/{t}/{c}/"),
                        None => format!("tok/{t}/"),
                    })
                    .collect();
                let mut refs: Vec<TokRef> = search::intersect(reader, &prefixes, DEFAULT_POSTING_CAP)?
                    .into_iter()
                    .filter_map(|hit| serde_json::from_slice(&hit.value).ok())
                    .collect();
                // newest first; (channel, seq) tiebreak for a stable order.
                refs.sort_by(|a, b| {
                    (b.time, &b.channel_id, b.seq).cmp(&(a.time, &a.channel_id, a.seq))
                });
                let limit = limit.unwrap_or(DEFAULT_SEARCH_LIMIT).clamp(1, MAX_SEARCH_LIMIT);
                let mut hits = Vec::new();
                for r in refs.into_iter().take(limit) {
                    if let Some(bytes) = reader.get(msg_key(&r.channel_id, r.seq).as_bytes())? {
                        let row: MsgRow = serde_json::from_slice(&bytes)
                            .map_err(|e| Error::Mapper(e.to_string()))?;
                        hits.push(row);
                    }
                }
                serde_json::to_vec(&ChatViewReply::Hits(hits))
                    .map_err(|e| Error::View(e.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chat_interface::encode_msg;
    use indexer::{AppliedOp, BlockOps, IndexStore};

    fn store(dir: &std::path::Path) -> IndexStore {
        IndexStore::open(dir, &["chat"])
            .expect("open store")
            .with_indexer(Box::new(ChatIndex::default()))
    }

    fn op(msg: &ChatMsg) -> AppliedOp {
        AppliedOp {
            module: "chat".into(),
            origin: OriginTag::external("jess"),
            payload: encode_msg(msg),
        }
    }

    fn post(channel: &str, id: &str, text: &str) -> AppliedOp {
        op(&ChatMsg::PostMessage {
            channel_id: channel.into(),
            message_id: id.into(),
            blocks: vec![Block::paragraph(text)],
            thread: None,
            as_agent: None,
        })
    }

    fn apply(store: &IndexStore, height: u64, ops: Vec<AppliedOp>) {
        store
            .apply_block(&BlockOps {
                height,
                time: 1_000 + height,
                ops,
            })
            .expect("apply");
    }

    fn search(store: &IndexStore, req: serde_json::Value) -> Vec<MsgRow> {
        let bytes = store
            .view("chat", &serde_json::to_vec(&req).unwrap())
            .expect("view");
        match serde_json::from_slice(&bytes).expect("reply decodes") {
            ChatViewReply::Hits(hits) => hits,
        }
    }

    #[test]
    fn posts_are_searchable_and_seq_mirrors() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        apply(&store, 1, vec![post("general", "m1", "hello fluent world")]);
        apply(&store, 2, vec![post("general", "m2", "unrelated words")]);
        apply(&store, 3, vec![post("random", "m3", "fluent chatter")]);

        let hits = search(&store, serde_json::json!({"search": {"text": "fluent"}}));
        assert_eq!(hits.len(), 2);
        // newest first.
        assert_eq!(hits[0].message_id, "m3");
        assert_eq!(hits[1].message_id, "m1");
        // per-channel sequences assigned in order.
        assert_eq!(hits[1].seq, 1);
        assert_eq!(hits[0].seq, 1);
        assert_eq!(hits[1].author, "user:jess");

        let hits = search(
            &store,
            serde_json::json!({"search": {"text": "fluent", "channelId": "general"}}),
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].message_id, "m1");
    }

    #[test]
    fn multi_token_search_is_an_and() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        apply(&store, 1, vec![post("g", "m1", "alpha beta gamma")]);
        apply(&store, 2, vec![post("g", "m2", "alpha delta")]);

        let hits = search(&store, serde_json::json!({"search": {"text": "alpha beta"}}));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].message_id, "m1");
    }

    #[test]
    fn edit_retokenizes_and_delete_tombstones() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        apply(&store, 1, vec![post("g", "m1", "original wording")]);
        apply(
            &store,
            2,
            vec![op(&ChatMsg::EditMessage {
                channel_id: "g".into(),
                seq: 1,
                blocks: vec![Block::paragraph("revised phrasing")],
                base_rev: None,
            })],
        );

        assert!(search(&store, serde_json::json!({"search": {"text": "original"}})).is_empty());
        let hits = search(&store, serde_json::json!({"search": {"text": "revised"}}));
        assert_eq!(hits.len(), 1);
        assert!(hits[0].edited);

        apply(
            &store,
            3,
            vec![op(&ChatMsg::DeleteMessage {
                channel_id: "g".into(),
                seq: 1,
            })],
        );
        assert!(search(&store, serde_json::json!({"search": {"text": "revised"}})).is_empty());
    }

    #[test]
    fn same_block_post_then_edit_folds_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        // one block: post assigns seq 1, edit of seq 1 lands right after —
        // the overlay must make the post's staged row visible to the edit.
        apply(
            &store,
            1,
            vec![
                post("g", "m1", "first draft"),
                op(&ChatMsg::EditMessage {
                    channel_id: "g".into(),
                    seq: 1,
                    blocks: vec![Block::paragraph("final text")],
                    base_rev: None,
                }),
            ],
        );
        assert!(search(&store, serde_json::json!({"search": {"text": "draft"}})).is_empty());
        assert_eq!(
            search(&store, serde_json::json!({"search": {"text": "final"}})).len(),
            1
        );
    }

    #[test]
    fn agent_author_is_rendered() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        let mut agent_post = post("g", "m1", "from an agent");
        agent_post.origin = OriginTag::module("agent");
        agent_post.payload = encode_msg(&ChatMsg::PostMessage {
            channel_id: "g".into(),
            message_id: "m1".into(),
            blocks: vec![Block::paragraph("from an agent")],
            thread: None,
            as_agent: Some("helper".into()),
        });
        apply(&store, 1, vec![agent_post]);
        let hits = search(&store, serde_json::json!({"search": {"text": "agent"}}));
        assert_eq!(hits[0].author, "agent:agent/helper");
    }
}
