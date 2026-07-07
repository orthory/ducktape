//! fold / query / parity tests for the derived tag index (`index/tags.rs`).
//! extraction unit tests live next to the extractor; these drive the whole
//! mapper through a real [`IndexStore`] exactly like `index.rs`'s own tests
//! (whose tiny harness is deliberately duplicated here rather than shared —
//! the two suites stay independently readable).

use super::{ChatViewReply, MsgRow, TagRow, tags};
use crate::{AuthorRef, Block, ChatMsg, ChatQuery, ChatReply, Span, encode_msg};
use indexer::{AppliedOp, BlockOps, Error, IndexStore, OriginTag};

fn store(dir: &std::path::Path) -> IndexStore {
    IndexStore::open(dir, &["chat"])
        .expect("open store")
        .with_indexer(Box::new(super::ChatIndex::default()))
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

fn edit(channel: &str, seq: u64, text: &str) -> AppliedOp {
    op(&ChatMsg::EditMessage {
        channel_id: channel.into(),
        seq,
        blocks: vec![Block::paragraph(text)],
        base_rev: None,
    })
}

fn delete(channel: &str, seq: u64) -> AppliedOp {
    op(&ChatMsg::DeleteMessage {
        channel_id: channel.into(),
        seq,
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

fn hits(store: &IndexStore, req: serde_json::Value) -> Vec<MsgRow> {
    let bytes = store
        .view("chat", &serde_json::to_vec(&req).unwrap())
        .expect("view");
    match serde_json::from_slice(&bytes).expect("reply decodes") {
        ChatViewReply::Hits(hits) => hits,
        other => panic!("expected hits, got {other:?}"),
    }
}

fn tag_rows(store: &IndexStore, req: serde_json::Value) -> Vec<TagRow> {
    let bytes = store
        .view("chat", &serde_json::to_vec(&req).unwrap())
        .expect("view");
    match serde_json::from_slice(&bytes).expect("reply decodes") {
        ChatViewReply::Tags(rows) => rows,
        other => panic!("expected tags, got {other:?}"),
    }
}

fn ids(rows: &[MsgRow]) -> Vec<&str> {
    rows.iter().map(|r| r.message_id.as_str()).collect()
}

// ── fold: post / edit / delete ──────────────────────────────────────────────

#[test]
fn posts_index_tags_and_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    apply(
        &store,
        1,
        vec![post("general", "m1", "shipping #rust today")],
    );
    apply(
        &store,
        2,
        vec![post("general", "m2", "more #rust and #wasm")],
    );
    apply(&store, 3, vec![post("random", "m3", "#rust elsewhere")]);

    // channel-scoped catalog: count desc, then tag asc; last_seq = newest live.
    let rows = tag_rows(
        &store,
        serde_json::json!({"tags": {"channelId": "general"}}),
    );
    assert_eq!(
        rows,
        vec![
            TagRow {
                tag: "rust".into(),
                count: 2,
                last_seq: 2
            },
            TagRow {
                tag: "wasm".into(),
                count: 1,
                last_seq: 2
            },
        ]
    );

    // no channel aggregates counts across channels; last_seq is the max of
    // the per-channel newest (seq spaces are per-channel).
    let rows = tag_rows(&store, serde_json::json!({"tags": {}}));
    assert_eq!(rows[0].tag, "rust");
    assert_eq!(rows[0].count, 3);
    assert_eq!(rows[0].last_seq, 2);

    // tag search: exact label, newest first, channel scope honored.
    let all = hits(&store, serde_json::json!({"tagSearch": {"tag": "rust"}}));
    assert_eq!(ids(&all), ["m3", "m2", "m1"]);
    let scoped = hits(
        &store,
        serde_json::json!({"tagSearch": {"tag": "rust", "channelId": "general"}}),
    );
    assert_eq!(ids(&scoped), ["m2", "m1"]);
    // rows carry their tag sets.
    assert_eq!(scoped[0].tags, ["rust", "wasm"]);
}

#[test]
fn tag_search_matches_exact_label_not_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    apply(&store, 1, vec![post("g", "m1", "#rust")]);
    apply(&store, 2, vec![post("g", "m2", "#rustlang")]);

    assert_eq!(
        ids(&hits(
            &store,
            serde_json::json!({"tagSearch": {"tag": "rust"}})
        )),
        ["m1"]
    );
    // the query normalizes: `#Rust` (as clicked in the app) finds `#rust`.
    assert_eq!(
        ids(&hits(
            &store,
            serde_json::json!({"tagSearch": {"tag": "#Rust"}})
        )),
        ["m1"]
    );
}

#[test]
fn hangul_tags_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    apply(&store, 1, vec![post("g", "m1", "이번 주 #한글-지원 작업")]);

    let rows = tag_rows(&store, serde_json::json!({"tags": {"channelId": "g"}}));
    assert_eq!(rows[0].tag, "한글-지원");
    assert_eq!(
        ids(&hits(
            &store,
            serde_json::json!({"tagSearch": {"tag": "#한글-지원"}})
        )),
        ["m1"]
    );
}

#[test]
fn code_blocks_and_link_spans_do_not_index_tags() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    apply(
        &store,
        1,
        vec![op(&ChatMsg::PostMessage {
            channel_id: "g".into(),
            message_id: "m1".into(),
            blocks: vec![
                Block::Code {
                    lang: None,
                    text: "#nope in code".into(),
                },
                Block::Paragraph(vec![Span {
                    text: "https://x.com/#nope".into(),
                    marks: vec![crate::Mark::Link("https://x.com/#nope".into())],
                }]),
                Block::paragraph("#yes"),
            ],
            thread: None,
            as_agent: None,
        })],
    );

    let rows = tag_rows(&store, serde_json::json!({"tags": {"channelId": "g"}}));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].tag, "yes");
    assert!(hits(&store, serde_json::json!({"tagSearch": {"tag": "nope"}})).is_empty());
}

#[test]
fn edit_diffs_old_and_new_tag_sets() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    apply(&store, 1, vec![post("g", "m1", "#old #keep")]);
    apply(&store, 2, vec![edit("g", 1, "#keep #new")]);

    assert!(hits(&store, serde_json::json!({"tagSearch": {"tag": "old"}})).is_empty());
    assert_eq!(
        ids(&hits(
            &store,
            serde_json::json!({"tagSearch": {"tag": "keep"}})
        )),
        ["m1"]
    );
    assert_eq!(
        ids(&hits(
            &store,
            serde_json::json!({"tagSearch": {"tag": "new"}})
        )),
        ["m1"]
    );
    // the catalog moved by the diff: `old` is gone at count zero.
    let rows = tag_rows(&store, serde_json::json!({"tags": {"channelId": "g"}}));
    assert_eq!(
        rows,
        vec![
            TagRow {
                tag: "keep".into(),
                count: 1,
                last_seq: 1
            },
            TagRow {
                tag: "new".into(),
                count: 1,
                last_seq: 1
            },
        ]
    );
}

#[test]
fn delete_removes_postings_and_decrements_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    apply(&store, 1, vec![post("g", "m1", "#x first")]);
    apply(&store, 2, vec![post("g", "m2", "#x second")]);
    apply(&store, 3, vec![delete("g", 2)]);

    // the NEWEST tagged message died: count decrements AND last_seq falls
    // back to the surviving posting (the catalog stores no last_seq — it is
    // read off the newest live posting, so no stale maximum survives).
    let rows = tag_rows(&store, serde_json::json!({"tags": {"channelId": "g"}}));
    assert_eq!(
        rows,
        vec![TagRow {
            tag: "x".into(),
            count: 1,
            last_seq: 1
        }]
    );
    assert_eq!(
        ids(&hits(
            &store,
            serde_json::json!({"tagSearch": {"tag": "x"}})
        )),
        ["m1"]
    );

    // deleting the last carrier drops the catalog entry entirely.
    apply(&store, 4, vec![delete("g", 1)]);
    assert!(tag_rows(&store, serde_json::json!({"tags": {"channelId": "g"}})).is_empty());
    assert!(hits(&store, serde_json::json!({"tagSearch": {"tag": "x"}})).is_empty());
}

#[test]
fn same_block_post_then_edit_folds_tags_through_the_overlay() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    apply(
        &store,
        1,
        vec![
            post("g", "m1", "#draft words"),
            edit("g", 1, "#final words"),
        ],
    );
    assert!(hits(&store, serde_json::json!({"tagSearch": {"tag": "draft"}})).is_empty());
    assert_eq!(
        ids(&hits(
            &store,
            serde_json::json!({"tagSearch": {"tag": "final"}})
        )),
        ["m1"]
    );
    let rows = tag_rows(&store, serde_json::json!({"tags": {"channelId": "g"}}));
    assert_eq!(
        rows,
        vec![TagRow {
            tag: "final".into(),
            count: 1,
            last_seq: 1
        }]
    );
}

#[test]
fn sixteen_tag_cap_binds_at_the_fold() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let text: String = (0..20).map(|i| format!("#tag{i:02} ")).collect();
    apply(&store, 1, vec![post("g", "m1", &text)]);

    let rows = tag_rows(
        &store,
        serde_json::json!({"tags": {"channelId": "g", "limit": 100}}),
    );
    assert_eq!(rows.len(), tags::MAX_TAGS_PER_MESSAGE);
    assert_eq!(
        hits(&store, serde_json::json!({"tagSearch": {"tag": "tag15"}})).len(),
        1
    );
    assert!(hits(&store, serde_json::json!({"tagSearch": {"tag": "tag16"}})).is_empty());
}

// ── queries: clamps and validation ──────────────────────────────────────────

#[test]
fn limits_default_and_clamp_like_search() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    for i in 0..25u64 {
        apply(
            &store,
            1 + i,
            vec![post("g", &format!("m{i}"), "#hot take")],
        );
    }

    // default page = 20, newest first.
    let page = hits(&store, serde_json::json!({"tagSearch": {"tag": "hot"}}));
    assert_eq!(page.len(), 20);
    assert_eq!(page[0].message_id, "m24");
    // zero clamps up to one, oversize clamps down to the max (100 ≥ 25 = all).
    assert_eq!(
        hits(
            &store,
            serde_json::json!({"tagSearch": {"tag": "hot", "limit": 0}})
        )
        .len(),
        1
    );
    assert_eq!(
        hits(
            &store,
            serde_json::json!({"tagSearch": {"tag": "hot", "limit": 100000}})
        )
        .len(),
        25
    );

    // the catalog query clamps the same way.
    let many: String = (0..3).map(|i| format!("#t{i} ")).collect();
    apply(&store, 100, vec![post("g", "mt", &many)]);
    assert_eq!(
        tag_rows(&store, serde_json::json!({"tags": {"limit": 0}})).len(),
        1
    );
}

#[test]
fn invalid_tag_queries_are_view_errors() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    apply(&store, 1, vec![post("g", "m1", "#ok")]);
    let long = "a".repeat(65);
    for bad in ["", "#", "two words", long.as_str()] {
        let req = serde_json::json!({"tagSearch": {"tag": bad}});
        let err = store
            .view("chat", &serde_json::to_vec(&req).unwrap())
            .unwrap_err();
        assert!(
            matches!(err, Error::View(_)),
            "tag {bad:?} should be a view error"
        );
    }
}

// ── fold vs rebuild parity ──────────────────────────────────────────────────

/// canonical chat state standing in for the module's query surface, paging
/// two views at a time like `index.rs`'s rebuild harness.
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
    }
}

#[allow(clippy::too_many_arguments)]
fn canonical_view(
    channel: &str,
    seq: u64,
    head_seq: u64,
    message_id: &str,
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
            author: AuthorRef::User(b"jess".to_vec()),
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
async fn rebuild_reproduces_the_folded_tag_index() {
    // store A: the tag state folds op by op, including an edit that swaps a
    // tag set and a delete of a tag's newest carrier.
    let dir_a = tempfile::tempdir().unwrap();
    let folded = store(dir_a.path());
    apply(&folded, 1, vec![post("g", "m1", "#alpha #beta launch")]);
    apply(&folded, 2, vec![post("g", "m2", "#gamma interim")]);
    apply(&folded, 3, vec![edit("g", 2, "#alpha interim")]);
    apply(&folded, 4, vec![post("g", "m3", "#alpha again")]);
    apply(&folded, 5, vec![delete("g", 3)]);
    apply(&folded, 6, vec![post("q", "m4", "#alpha crosses channels")]);

    // store B: rebuilt from canonical heads equal to A's FINAL live state.
    // created_at mirrors A's fold times so newest-first ranking agrees.
    let dir_b = tempfile::tempdir().unwrap();
    let rebuilt = store(dir_b.path());
    let state = CanonicalChat {
        channels: vec![canonical_channel("g", 3), canonical_channel("q", 1)],
        views: vec![
            canonical_view("g", 1, 3, "m1", "#alpha #beta launch", 1_001, 0, false),
            canonical_view("g", 2, 3, "m2", "#alpha interim", 1_002, 1, false),
            canonical_view("g", 3, 3, "m3", "", 1_004, 0, true),
            canonical_view("q", 1, 1, "m4", "#alpha crosses channels", 1_006, 0, false),
        ],
    };
    rebuilt
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

    // the catalog re-derives identically — per channel and aggregated.
    for req in [
        serde_json::json!({"tags": {"channelId": "g"}}),
        serde_json::json!({"tags": {"channelId": "q"}}),
        serde_json::json!({"tags": {}}),
    ] {
        assert_eq!(
            tag_rows(&folded, req.clone()),
            tag_rows(&rebuilt, req),
            "catalog parity"
        );
    }
    assert_eq!(
        tag_rows(&folded, serde_json::json!({"tags": {"channelId": "g"}})),
        vec![
            TagRow {
                tag: "alpha".into(),
                count: 2,
                last_seq: 2
            },
            TagRow {
                tag: "beta".into(),
                count: 1,
                last_seq: 1
            },
        ]
    );

    // the postings re-derive an exact hit set (rows differ only by `height`,
    // the rebuild's NAMED degradation — compare identity + tag sets).
    for tag in ["alpha", "beta", "gamma"] {
        let req = serde_json::json!({"tagSearch": {"tag": tag}});
        let a: Vec<_> = hits(&folded, req.clone())
            .into_iter()
            .map(|r| {
                (
                    r.channel_id,
                    r.seq,
                    r.message_id,
                    r.time,
                    r.tags,
                    r.edited,
                    r.deleted,
                )
            })
            .collect();
        let b: Vec<_> = hits(&rebuilt, req)
            .into_iter()
            .map(|r| {
                (
                    r.channel_id,
                    r.seq,
                    r.message_id,
                    r.time,
                    r.tags,
                    r.edited,
                    r.deleted,
                )
            })
            .collect();
        assert_eq!(a, b, "posting parity for #{tag}");
    }
    assert_eq!(
        ids(&hits(
            &folded,
            serde_json::json!({"tagSearch": {"tag": "alpha"}})
        )),
        ["m4", "m2", "m1"]
    );
}
