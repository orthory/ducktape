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
//! caveat (shared by every mapper): the view reflects ops applied SINCE the
//! index existed. enabling an index against storage with prior chat history
//! leaves that history unsearchable — and the seq mirror offset — until the
//! index is re-derived, either by replaying the chain from genesis or via the
//! from-state rebuild below.
//!
//! from-state rebuild: canonical `Channels`/`MessagesRange` enumerate every
//! sequence gap-free (tombstones and replies included), so the seq mirrors,
//! rows, and postings all re-derive with an exact hit set. this mapper is the
//! spec's NAMED degradation case: canonical heads keep no block height, so
//! `height` collapses to the boundary — but `created_at` survives, so `time`
//! (and with it search ranking) stays exact.

use crate::{
    AuthorRef, Block, ChatMsg, ChatQuery, ChatReply, DEFAULT_CHAT_TARGET, MAX_QUERY_LIMIT, Span,
    decode_msg, decode_reply, encode_query,
};
use indexer::search::{self, DEFAULT_POSTING_CAP};
use indexer::{
    ApplyCtx, Backfill, Derived, Error, ModuleIndexer, OpMeta, OriginKind, OriginTag, RebuildMeta,
    Result, StateReader, ViewReader,
};
use serde::{Deserialize, Serialize};

mod tags;
pub use tags::{MAX_TAG_CHARS, MAX_TAGS_PER_MESSAGE, TagRow};

/// default and max page size for search results.
const DEFAULT_SEARCH_LIMIT: usize = 20;
const MAX_SEARCH_LIMIT: usize = 100;

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

/// render a canonical [`AuthorRef`] to the SAME string [`author`] renders the
/// dispatch origin to — rebuilt rows must read identically to folded ones.
/// user keys go through [`indexer::user_handle`], the same rendering the node
/// layer applies when it flattens an external origin into an [`OriginTag`]
/// (`noded::index_origin`): printable names pass through, raw pubkeys become
/// hex — never the lossy `�` boxes a plain utf-8 decode would leave.
fn author_from_ref(author: &AuthorRef) -> String {
    match author {
        AuthorRef::User(key) => format!("user:{}", indexer::user_handle(key)),
        AuthorRef::Agent { module, agent_id } => format!("agent:{module}/{agent_id}"),
        AuthorRef::Module(id) => format!("module:{id}"),
        AuthorRef::System => "system".to_string(),
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

/// every entry one head state materializes to — the row plus one posting per
/// token and per tag label (a tombstone's empty text and tag set yield none).
/// fold and rebuild both write THROUGH this, so the two paths produce
/// byte-identical rows.
fn row_entries(row: &MsgRow) -> Result<Vec<(String, Vec<u8>)>> {
    let mut entries = vec![(
        msg_key(&row.channel_id, row.seq),
        serde_json::to_vec(row).map_err(|e| Error::Mapper(e.to_string()))?,
    )];
    let tok_ref = serde_json::to_vec(&TokRef {
        channel_id: row.channel_id.clone(),
        seq: row.seq,
        message_id: row.message_id.clone(),
        time: row.time,
    })
    .map_err(|e| Error::Mapper(e.to_string()))?;
    for token in search::tokens(&row.text) {
        entries.push((tok_key(&token, &row.channel_id, row.seq), tok_ref.clone()));
    }
    for label in &row.tags {
        entries.push((
            tags::tag_key(label, &row.channel_id, row.seq),
            tok_ref.clone(),
        ));
    }
    Ok(entries)
}

fn put_row_and_toks(out: &mut Derived, row: &MsgRow) -> Result<()> {
    for (key, value) in row_entries(row)? {
        out.put(key, value);
    }
    Ok(())
}

fn delete_toks(out: &mut Derived, row: &MsgRow) {
    for token in search::tokens(&row.text) {
        out.delete(tok_key(&token, &row.channel_id, row.seq));
    }
}

#[async_trait::async_trait(?Send)]
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
                    tags: tags::labels(&blocks),
                    channel_id,
                };
                tags::fold_catalog(ctx, out, &row.channel_id, &[], &row.tags)?;
                put_row_and_toks(out, &row)
            }
            ChatMsg::EditMessage {
                channel_id,
                seq,
                blocks,
                ..
            } => {
                // absent row == the message predates this index; nothing to
                // retokenize (see the module doc's pre-index caveat).
                let Some(mut row) = read_row(ctx, &msg_key(&channel_id, seq))? else {
                    return Ok(());
                };
                if row.deleted {
                    return Ok(());
                }
                // delete BEFORE re-putting: tokens/tags shared by the old and
                // new text stage a delete then a put, and the last action
                // wins. the catalog moves by the old/new tag-set DIFF.
                delete_toks(out, &row);
                tags::delete_postings(out, &row);
                let new_tags = tags::labels(&blocks);
                tags::fold_catalog(ctx, out, &row.channel_id, &row.tags, &new_tags)?;
                row.text = plain_text(&blocks);
                row.tags = new_tags;
                row.edited = true;
                put_row_and_toks(out, &row)
            }
            ChatMsg::DeleteMessage { channel_id, seq } => {
                let Some(mut row) = read_row(ctx, &msg_key(&channel_id, seq))? else {
                    return Ok(());
                };
                delete_toks(out, &row);
                tags::delete_postings(out, &row);
                tags::fold_catalog(ctx, out, &row.channel_id, &row.tags, &[])?;
                row.deleted = true;
                row.text = String::new();
                row.tags = Vec::new();
                put_row_and_toks(out, &row)
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
            | ChatMsg::SweepHuddle { .. } => Ok(()),
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
                let tokens: Vec<String> = search::tokens(&text).into_iter().collect();
                if tokens.is_empty() {
                    return Err(Error::View("search text has no tokens".into()));
                }
                // each token matches as a prefix; the channel scope filters the
                // intersected refs by their stored channel (postings can't embed
                // it after a partial token).
                let mut refs: Vec<TokRef> =
                    search::intersect_prefix(reader, "tok/", &tokens, DEFAULT_POSTING_CAP)?
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
                    if let Some(bytes) = reader.get(msg_key(&r.channel_id, r.seq).as_bytes())? {
                        let row: MsgRow = serde_json::from_slice(&bytes)
                            .map_err(|e| Error::Mapper(e.to_string()))?;
                        hits.push(row);
                    }
                }
                serde_json::to_vec(&ChatViewReply::Hits(hits))
                    .map_err(|e| Error::View(e.to_string()))
            }
            ChatViewQuery::Tags { channel_id, limit } => {
                let rows = tags::serve_tags(reader, channel_id, limit)?;
                serde_json::to_vec(&ChatViewReply::Tags(rows))
                    .map_err(|e| Error::View(e.to_string()))
            }
            ChatViewQuery::TagSearch {
                tag,
                channel_id,
                limit,
            } => {
                let hits = tags::serve_tag_search(reader, &tag, channel_id, limit)?;
                serde_json::to_vec(&ChatViewReply::Hits(hits))
                    .map_err(|e| Error::View(e.to_string()))
            }
        }
    }

    fn supports_rebuild(&self) -> bool {
        true
    }

    /// re-derive the seq mirrors, rows, and postings from canonical
    /// `Channels`/`MessagesRange`. the per-channel sequence space is gap-free
    /// (tombstones and replies included), so every head re-derives. the
    /// spec's named degradation: heads keep no block height, so `height`
    /// collapses to the boundary — `time` survives via `created_at`, so
    /// ranking stays exact.
    async fn rebuild_from_state(
        &self,
        state: &dyn StateReader,
        meta: &RebuildMeta,
        out: &mut Backfill<'_>,
    ) -> Result<()> {
        let reply = state.query(&encode_query(&ChatQuery::Channels)).await?;
        let channels = match decode_reply(&reply).map_err(Error::State)? {
            ChatReply::Channels(channels) => channels,
            other => return Err(Error::State(format!("Channels answered {other:?}"))),
        };
        for channel in channels {
            out.put(
                seq_key(&channel.id),
                channel.head_seq.to_be_bytes().to_vec(),
            )?;
            // the channel's tag catalog re-accumulates from the live heads —
            // count-only, exactly what the fold maintains incrementally.
            let mut tag_counts: std::collections::BTreeMap<String, u64> =
                std::collections::BTreeMap::new();
            let mut from_seq = 1u64;
            while from_seq <= channel.head_seq {
                let reply = state
                    .query(&encode_query(&ChatQuery::MessagesRange {
                        channel_id: channel.id.clone(),
                        from_seq,
                        limit: MAX_QUERY_LIMIT,
                    }))
                    .await?;
                let views = match decode_reply(&reply).map_err(Error::State)? {
                    ChatReply::Messages(views) => views,
                    other => {
                        return Err(Error::State(format!("MessagesRange answered {other:?}")));
                    }
                };
                // the sequence space is gap-free through head_seq, so an
                // empty page below the head is drift, not the end.
                let Some(last) = views.last() else {
                    return Err(Error::State(format!(
                        "channel {} empty at seq {from_seq}, head {}",
                        channel.id, channel.head_seq
                    )));
                };
                from_seq = last.seq + 1;
                for view in views {
                    let head = view.head;
                    let row = MsgRow {
                        channel_id: view.channel_id,
                        seq: view.seq,
                        message_id: head.message_id,
                        author: author_from_ref(&head.author),
                        height: meta.height,
                        time: head.created_at,
                        // mirror the fold's tombstone exactly: empty text and
                        // tag set (so no postings), whatever skeleton the
                        // head kept.
                        text: if head.deleted {
                            String::new()
                        } else {
                            plain_text(&head.blocks)
                        },
                        deleted: head.deleted,
                        edited: head.rev > 0,
                        thread: head.thread,
                        tags: if head.deleted {
                            Vec::new()
                        } else {
                            tags::labels(&head.blocks)
                        },
                    };
                    for label in &row.tags {
                        *tag_counts.entry(label.clone()).or_insert(0) += 1;
                    }
                    for (key, value) in row_entries(&row)? {
                        out.put(key, value)?;
                    }
                }
            }
            for (label, count) in tag_counts {
                out.put(
                    tags::catalog_key(&channel.id, &label),
                    tags::encode_catalog(count)?,
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tag_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode_msg;
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
                record: None,
            })
            .expect("apply");
    }

    fn search(store: &IndexStore, req: serde_json::Value) -> Vec<MsgRow> {
        let bytes = store
            .view("chat", &serde_json::to_vec(&req).unwrap())
            .expect("view");
        match serde_json::from_slice(&bytes).expect("reply decodes") {
            ChatViewReply::Hits(hits) => hits,
            other => panic!("expected hits, got {other:?}"),
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
            serde_json::json!({"search": {"text": "fluent", "channel_id": "general"}}),
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

        let hits = search(
            &store,
            serde_json::json!({"search": {"text": "alpha beta"}}),
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].message_id, "m1");
    }

    #[test]
    fn partial_tokens_match_as_a_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        apply(&store, 1, vec![post("general", "m1", "testing the waters")]);

        // typing a partial word surfaces the message before the full word is
        // typed — the command-palette search-as-you-type contract.
        for q in ["te", "tes", "test", "testing"] {
            let hits = search(&store, serde_json::json!({"search": {"text": q}}));
            assert_eq!(hits.len(), 1, "query {q:?} should match `testing`");
            assert_eq!(hits[0].message_id, "m1");
        }
        // a prefix that matches no word still finds nothing.
        assert!(search(&store, serde_json::json!({"search": {"text": "xyz"}})).is_empty());
    }

    #[test]
    fn prefix_search_is_still_an_and_across_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        apply(&store, 1, vec![post("g", "m1", "alpha beta gamma")]);
        apply(&store, 2, vec![post("g", "m2", "alpha delta")]);

        // each token is a prefix; ALL must match some word in the message.
        let hits = search(&store, serde_json::json!({"search": {"text": "alp bet"}}));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].message_id, "m1");
    }

    #[test]
    fn prefix_match_dedups_multiple_words_per_message() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        // three distinct words all share the prefix — one message, one hit.
        apply(&store, 1, vec![post("g", "m1", "test tester testing")]);
        let hits = search(&store, serde_json::json!({"search": {"text": "test"}}));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].message_id, "m1");
    }

    #[test]
    fn prefix_search_honors_channel_scope() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        apply(&store, 1, vec![post("general", "m1", "testing here")]);
        apply(&store, 2, vec![post("random", "m2", "testing there")]);

        let hits = search(
            &store,
            serde_json::json!({"search": {"text": "test", "channel_id": "general"}}),
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].message_id, "m1");
    }

    #[test]
    fn user_author_renders_binary_key_as_hex_not_garbage() {
        // a raw ed25519-style key: 32 bytes that are NOT printable utf-8.
        let key: Vec<u8> = (0u8..32)
            .map(|i| i.wrapping_mul(37).wrapping_add(0x80))
            .collect();
        let rendered = author_from_ref(&AuthorRef::User(key.clone()));
        let handle = rendered.strip_prefix("user:").expect("user-tagged");
        assert!(
            !handle.contains('\u{FFFD}'),
            "no lossy replacement chars: {handle:?}"
        );
        assert!(
            handle.chars().all(|c| !c.is_control()),
            "no control chars: {handle:?}"
        );
        // it is the hex of the key bytes.
        assert_eq!(handle, indexer::user_handle(&key));
        // a printable claimed name (embedded daemon) still passes through.
        assert_eq!(
            author_from_ref(&AuthorRef::User(b"jess".to_vec())),
            "user:jess"
        );
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

    /// canonical chat state standing in for the module's query surface. pages
    /// `MessagesRange` TWO views at a time regardless of the asked limit, so
    /// the rebuild's pagination loop is exercised, not just its first lap.
    struct CanonicalChat {
        channels: Vec<crate::Channel>,
        views: Vec<crate::MessageView>,
    }

    #[async_trait::async_trait(?Send)]
    impl indexer::StateReader for CanonicalChat {
        async fn query(&self, req: &[u8]) -> indexer::Result<Vec<u8>> {
            let reply = match crate::decode_query(req).map_err(Error::State)? {
                ChatQuery::Channels => ChatReply::Channels(self.channels.clone()),
                ChatQuery::MessagesRange {
                    channel_id,
                    from_seq,
                    ..
                } => ChatReply::Messages(
                    self.views
                        .iter()
                        .filter(|v| v.channel_id == channel_id && v.seq >= from_seq)
                        .take(2)
                        .cloned()
                        .collect(),
                ),
                other => return Err(Error::State(format!("unexpected query {other:?}"))),
            };
            Ok(crate::encode_reply(&reply))
        }
    }

    fn canonical_channel(id: &str, head_seq: u64) -> crate::Channel {
        crate::Channel {
            id: id.into(),
            name: id.into(),
            created_at: 900,
            head_seq,
            post_policy: crate::PostPolicy::Open,
            hooks: Vec::new(),
            pinned: Vec::new(),
            huddle: Vec::new(),
            owner: None,
            archived: false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn canonical_view(
        channel: &str,
        seq: u64,
        head_seq: u64,
        message_id: &str,
        author: AuthorRef,
        text: &str,
        created_at: u64,
        rev: u32,
        deleted: bool,
    ) -> crate::MessageView {
        crate::MessageView {
            channel_id: channel.into(),
            seq,
            head: crate::MessageHead {
                message_id: message_id.into(),
                author,
                blocks: vec![Block::paragraph(text)],
                created_at,
                rev,
                edited_at: (rev > 0).then_some(created_at + 1),
                base_rev: None,
                deleted,
                thread: None,
                reply_count: 0,
                last_reply_seq: None,
            },
            reactions: Vec::new(),
            channel_head_seq: head_seq,
        }
    }

    #[tokio::test]
    async fn rebuild_rederives_channels_pagination_and_tombstones() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        // folded rows the rebuild must throw away.
        apply(&store, 1, vec![post("stale", "m0", "vanishing text")]);

        // channel g: 3 live sequences (pages of 2 exercise the loop), one
        // edited; channel q: one tombstone.
        let state = CanonicalChat {
            channels: vec![canonical_channel("g", 3), canonical_channel("q", 1)],
            views: vec![
                canonical_view(
                    "g",
                    1,
                    3,
                    "m1",
                    AuthorRef::User(b"jess".to_vec()),
                    "hello fluent world",
                    1_001,
                    0,
                    false,
                ),
                canonical_view(
                    "g",
                    2,
                    3,
                    "m2",
                    AuthorRef::Agent {
                        module: "agent".into(),
                        agent_id: "helper".into(),
                    },
                    "fluent chatter",
                    1_002,
                    0,
                    false,
                ),
                canonical_view(
                    "g",
                    3,
                    3,
                    "m3",
                    AuthorRef::User(b"eddy".to_vec()),
                    "revised phrasing",
                    1_003,
                    2,
                    false,
                ),
                canonical_view(
                    "q",
                    1,
                    1,
                    "m4",
                    AuthorRef::User(b"jess".to_vec()),
                    "was deleted",
                    1_004,
                    0,
                    true,
                ),
            ],
        };
        store
            .rebuild_module(
                "chat",
                &state,
                indexer::RebuildMeta {
                    height: 50,
                    time: 0,
                },
            )
            .await
            .expect("rebuild");

        assert!(
            search(&store, serde_json::json!({"search": {"text": "vanishing"}})).is_empty(),
            "pre-rebuild rows do not survive"
        );

        let hits = search(&store, serde_json::json!({"search": {"text": "fluent"}}));
        assert_eq!(hits.len(), 2);
        // ranking survives the rebuild: created_at is canonical.
        assert_eq!(hits[0].message_id, "m2");
        assert_eq!(hits[0].author, "agent:agent/helper");
        assert_eq!(hits[1].author, "user:jess");
        assert_eq!(hits[1].time, 1_001, "time survives via created_at");
        assert_eq!(hits[1].height, 50, "height collapses to the boundary");

        let hits = search(&store, serde_json::json!({"search": {"text": "revised"}}));
        assert_eq!(hits.len(), 1);
        assert!(hits[0].edited, "rev > 0 rebuilds as edited");

        assert!(
            search(&store, serde_json::json!({"search": {"text": "deleted"}})).is_empty(),
            "tombstones rebuild without postings"
        );

        assert_eq!(store.applied_height("chat").unwrap(), 50);
        assert_eq!(store.backfill_height("chat").unwrap(), Some(50));

        // the rebuilt seq mirror carries the fold forward: the next post in g
        // is assigned seq 4, and an edit of a rebuilt row retokenizes.
        apply(&store, 51, vec![post("g", "m5", "post rebuild message")]);
        let hits = search(&store, serde_json::json!({"search": {"text": "rebuild"}}));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].seq, 4, "seq mirror rebuilt from head_seq");
        apply(
            &store,
            52,
            vec![op(&ChatMsg::EditMessage {
                channel_id: "g".into(),
                seq: 3,
                blocks: vec![Block::paragraph("polished phrasing")],
                base_rev: None,
            })],
        );
        assert!(search(&store, serde_json::json!({"search": {"text": "revised"}})).is_empty());
        assert_eq!(
            search(&store, serde_json::json!({"search": {"text": "polished"}})).len(),
            1
        );
    }
}
