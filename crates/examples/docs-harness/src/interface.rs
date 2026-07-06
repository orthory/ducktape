//! the docs-harness module's public wire surface — types only, no sdk dep.
//!
//! the reference Quack package's harness (design D9) owns three open action
//! tags against the pages module. their payloads are THIS surface: the tag's
//! owning module is the schema authority, so the JSON schema files shipped in
//! `packages/docs/actions/` describe exactly these shapes. everything here is
//! serde_json on the wire, like every module interface.

use serde::{Deserialize, Serialize};

/// add a comment: open a new thread on `target` (no `thread_id`) or append to
/// an existing one. see [`CommentAddPayload`].
pub const ACTION_COMMENT_ADD: &str = "pages.comment.add";
/// replace one block's text, optionally guarded by the block's prior content
/// hash. see [`BlockUpdateTextPayload`].
pub const ACTION_BLOCK_UPDATE_TEXT: &str = "pages.block.update_text";
/// resolve or reopen a comment thread. see [`ThreadResolvePayload`].
pub const ACTION_THREAD_RESOLVE: &str = "pages.thread.resolve";

// ---- write-time caps (consensus constants) ----------------------------------
// tighter than the pages module's own caps on purpose: the harness is the
// action's schema authority, and a bound it enforces at probe time can never
// abort the delivery block later.

/// comment text cap for [`ACTION_COMMENT_ADD`] (the schema's `maxLength`).
pub const MAX_COMMENT_TEXT_BYTES: usize = 4096;
/// block text cap for [`ACTION_BLOCK_UPDATE_TEXT`] (the schema's `maxLength`).
pub const MAX_BLOCK_TEXT_BYTES: usize = 64 * 1024;
/// an action id enters the minted comment/thread ids, so it is length-gated.
pub const MAX_ACTION_ID_BYTES: usize = 128;
/// committed failure rows kept for audit; the oldest is evicted beyond this.
pub const MAX_FAILURE_ROWS: usize = 64;

// ---- action payloads ----------------------------------------------------------
// strict by construction (`deny_unknown_fields`): an action payload either IS
// the declared schema or the probe rejects it.

/// `pages.comment.add`: anchor to `target` (a block or page id). with
/// `thread_id`, append to that existing thread; without, open a new thread
/// under a deterministically minted id (see [`minted_thread_id`]).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CommentAddPayload {
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    pub text: String,
}

/// `pages.block.update_text`: replace `block_id`'s text. `expected_hash`
/// (`"sha256:<64 lowercase hex>"` over the block's CURRENT text bytes) is the
/// concurrent-edit guard: a mismatch records a failure instead of clobbering.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BlockUpdateTextPayload {
    pub block_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_hash: Option<String>,
    pub text: String,
}

/// `pages.thread.resolve`: set an existing thread's resolved flag.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ThreadResolvePayload {
    pub thread_id: String,
    pub resolved: bool,
}

/// the `run_context` a probe/apply carries (the package contract:
/// `{ run_id, agent_id, package? }`). lenient: later fields are ignored.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RunContext {
    pub run_id: String,
    pub agent_id: String,
}

/// the deterministic thread id a no-`thread_id` comment action opens.
pub fn minted_thread_id(run_id: &str, action_id: &str) -> String {
    format!("docs:t:{run_id}:{action_id}")
}

/// the deterministic comment id every comment action mints.
pub fn minted_comment_id(run_id: &str, action_id: &str) -> String {
    format!("docs:c:{run_id}:{action_id}")
}

/// the job id one (agent, comment) engagement mints — also the idempotency
/// anchor: a redelivered event maps to the same id.
pub fn engagement_job_id(agent_id: &str, comment_id: &str) -> String {
    format!("docs:{agent_id}:{comment_id}")
}

/// the spec a minted `agent/<agent_id>` job carries: where the mention lives,
/// as one serde_json object (deterministic field order — struct order).
///
/// `text` is a bounded EXCERPT of the mentioning comment (at most
/// [`MAX_COMMENT_TEXT_BYTES`], cut at a char boundary — see
/// [`engagement_excerpt`]), never the full text: pages allows comments up to
/// its own 64 KiB cap, and a full near-cap (or escape-heavy) copy could push
/// the encoded spec past the jobs board's spec cap — aborting the COMMENTER's
/// block from the no-fail intake arm. the agent reads the full comment from
/// pages at run time via `comment_id`; the excerpt is orientation, not truth.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EngagementSpec {
    pub page_id: String,
    pub target: String,
    pub thread_id: String,
    pub comment_id: String,
    pub text: String,
}

/// the bounded excerpt an [`EngagementSpec`] carries: the comment's leading
/// bytes up to [`MAX_COMMENT_TEXT_BYTES`], cut back to the nearest char
/// boundary so the excerpt stays valid utf-8.
pub fn engagement_excerpt(text: &str) -> String {
    if text.len() <= MAX_COMMENT_TEXT_BYTES {
        return text.to_owned();
    }
    let mut end = MAX_COMMENT_TEXT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

pub fn encode_engagement_spec(s: &EngagementSpec) -> String {
    serde_json::to_string(s).expect("serializable")
}

pub fn decode_engagement_spec(s: &str) -> Result<EngagementSpec, String> {
    serde_json::from_str(s).map_err(|e| e.to_string())
}

// ---- queries -------------------------------------------------------------------

/// one recorded action failure (the error-row surface behind "mutate nothing,
/// record failure"): which action, which tag, why.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FailureRow {
    pub action_id: String,
    pub tag: String,
    pub reason: String,
}

/// the harness's committed lifecycle view.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DocsStatus {
    pub package: String,
    /// `"active"`, `"suspended"`, or `"unplugged"`.
    pub phase: String,
    pub agents: Vec<String>,
    /// how many jobs this harness has minted over its life.
    pub minted: u64,
    /// how many failure rows are currently retained (bounded).
    pub failures: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocsQuery {
    Status,
    Failures,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocsReply {
    Status(Option<DocsStatus>),
    Failures(Vec<FailureRow>),
}

pub fn encode_query(q: &DocsQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<DocsQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &DocsReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<DocsReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

#[cfg(test)]
mod interface_tests {
    use super::*;

    #[test]
    fn payloads_round_trip_and_reject_unknown_fields() {
        let c = CommentAddPayload {
            target: "b1".into(),
            thread_id: None,
            text: "hi".into(),
        };
        let bytes = serde_json::to_vec(&c).unwrap();
        assert_eq!(
            serde_json::from_slice::<CommentAddPayload>(&bytes).unwrap(),
            c
        );
        // optional fields serialize away entirely.
        assert!(!String::from_utf8(bytes).unwrap().contains("thread_id"));

        for (json, ok) in [
            (r#"{"target":"b1","text":"hi"}"#, true),
            (r#"{"target":"b1","text":"hi","thread_id":"t1"}"#, true),
            (r#"{"target":"b1","text":"hi","bogus":1}"#, false),
        ] {
            assert_eq!(
                serde_json::from_str::<CommentAddPayload>(json).is_ok(),
                ok,
                "{json}"
            );
        }
        assert!(
            serde_json::from_str::<BlockUpdateTextPayload>(
                r#"{"block_id":"b1","text":"x","page_id":"p1"}"#
            )
            .is_err(),
            "unknown fields reject"
        );
    }

    #[test]
    fn run_context_is_lenient() {
        let rc: RunContext =
            serde_json::from_str(r#"{"run_id":"r1","agent_id":"a","package":"later"}"#).unwrap();
        assert_eq!(rc.run_id, "r1");
    }

    #[test]
    fn engagement_excerpts_are_bounded_and_char_clean() {
        // under the cap: verbatim.
        let short = "hello @docs.editor";
        assert_eq!(engagement_excerpt(short), short);
        // exactly at the cap: verbatim.
        let exact = "a".repeat(MAX_COMMENT_TEXT_BYTES);
        assert_eq!(engagement_excerpt(&exact), exact);
        // past the cap with 3-byte chars: 4096 % 3 == 1, so the raw cut would
        // split a char — the excerpt backs off to the nearest boundary.
        let long = "한".repeat(MAX_COMMENT_TEXT_BYTES);
        let cut = engagement_excerpt(&long);
        assert_eq!(
            cut.len(),
            MAX_COMMENT_TEXT_BYTES - MAX_COMMENT_TEXT_BYTES % 3
        );
        assert!(long.starts_with(&cut));
    }

    #[test]
    fn minted_ids_are_deterministic() {
        assert_eq!(minted_thread_id("r1", "a1"), "docs:t:r1:a1");
        assert_eq!(minted_comment_id("r1", "a1"), "docs:c:r1:a1");
        assert_eq!(
            engagement_job_id("docs.editor", "c1"),
            "docs:docs.editor:c1"
        );
    }

    #[test]
    fn queries_and_specs_round_trip() {
        for q in [DocsQuery::Status, DocsQuery::Failures] {
            assert_eq!(decode_query(&encode_query(&q)).unwrap(), q);
        }
        let r = DocsReply::Failures(vec![FailureRow {
            action_id: "a1".into(),
            tag: ACTION_COMMENT_ADD.into(),
            reason: "unknown target".into(),
        }]);
        assert_eq!(decode_reply(&encode_reply(&r)).unwrap(), r);
        let spec = EngagementSpec {
            page_id: "p1".into(),
            target: "b1".into(),
            thread_id: "t1".into(),
            comment_id: "c1".into(),
            text: "@docs.editor hi".into(),
        };
        assert_eq!(
            decode_engagement_spec(&encode_engagement_spec(&spec)).unwrap(),
            spec
        );
    }
}
