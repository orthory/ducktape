//! the forge module's public wire surface — types only.
//!
//! forge is a git-backed module: its state is a NAMED NAMESPACE of git repos,
//! addressed by a repo slug (`[a-z0-9._-]`, 1..=64 bytes). its `root()` is a
//! canonical sorted hash over the committed HEAD oid of every repo that has a
//! head. writes go via [`ForgeMsg`] (a file put + commit, or a git push); reads
//! via [`ForgeQuery`] -> [`ForgeReply`], returning HEAD oids as hex.
//!
//! ## the default repo
//!
//! the `repo` field on every [`ForgeMsg`] variant carries
//! `#[serde(default)]`, so a wire message that omits it deserializes with
//! `repo == ""`; the module normalizes an empty repo to the well-known
//! `"default"` repo. a single-repo client that sends
//! `{"commit":{path,content,message}}` and queries `"head"` therefore keeps
//! targeting one canonical repo with no change — the multi-repo surface
//! ([`ForgeQuery::HeadOf`]/[`ForgeQuery::ListRepos`]) is purely additive.

use serde::{Deserialize, Serialize};

use crate::tracker_iface::{RefUpdate, ReviewComment, ReviewVerdict};

/// a write intent at forge.
///
/// the git surface: the file-by-file [`ForgeMsg::Commit`] (forge builds the
/// commit object itself) and the atomic multi-branch [`ForgeMsg::PushRefs`] —
/// git-faithful ref updates that adopt a client's REAL commit history by oid,
/// with the objects carried out-of-band in a node-local packfile (never in
/// consensus).
///
/// the tracker surface: GitHub-shaped issues / pull requests / reviews
/// ([`ForgeMsg::OpenIssue`] .. [`ForgeMsg::SubmitReview`]) — see
/// [`crate::tracker`].
///
/// every variant names its target repo via `repo`. the field is
/// `#[serde(default)]`, so an omitted/empty `repo` deserializes to `""` and the
/// module maps it to the `"default"` repo (see the module docstring) — the
/// single-repo wire needs no `repo` key at all.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForgeMsg {
    Commit {
        /// the target repo slug; empty/absent -> the `"default"` repo.
        #[serde(default)]
        repo: String,
        path: String,
        content: String,
        message: String,
    },
    /// the atomic multi-branch push: every [`RefUpdate`] is a per-branch CAS
    /// against that branch's COMMITTED head, and the whole list stages or the
    /// whole op rejects. `pack_digest` (sha256, 32 raw bytes) locates the ONE
    /// packfile carrying the closure of every updated head; a delete-only push
    /// carries `None`. this is what a stock `git push` lands as (the smart-HTTP
    /// bridge translates the command list).
    PushRefs {
        #[serde(default)]
        repo: String,
        updates: Vec<RefUpdate>,
        pack_digest: Option<Vec<u8>>,
    },
    /// open an issue. assigns the repo's next shared number, stores title/body
    /// on the record (authorship is origin-derived), and emits a follow-up
    /// creating the hidden discussion channel `forge:<repo>:<n>` — atomic with
    /// the record.
    OpenIssue {
        #[serde(default)]
        repo: String,
        title: String,
        #[serde(default)]
        body: String,
    },
    /// open a pull request from a born `source_branch` onto `target_branch`
    /// (empty -> "main"). same number space + discussion channel as issues.
    OpenPr {
        #[serde(default)]
        repo: String,
        title: String,
        #[serde(default)]
        body: String,
        source_branch: String,
        #[serde(default)]
        target_branch: String,
    },
    /// edit an item's title and/or body — author-only.
    EditItem {
        #[serde(default)]
        repo: String,
        number: u64,
        title: Option<String>,
        body: Option<String>,
    },
    /// close (`open: false`) or reopen (`open: true`) an item. merged PRs are
    /// terminal; an unchanged state is a deterministic no-op.
    SetItemState {
        #[serde(default)]
        repo: String,
        number: u64,
        open: bool,
    },
    /// merge an open PR. the merge commit is CLIENT-COMPUTED (validators may
    /// not hold the objects — same trust model as `PushRefs`): the merging client
    /// builds it locally, uploads its pack, then submits this op. consensus
    /// gates on a double CAS — the target branch must still be at
    /// `prev_target_oid` AND the source at `expected_source_oid` — then moves
    /// the target to `merge_oid` and marks the PR merged, atomically. oids are
    /// 40-char sha1 hex; `pack_digest` is 64-char sha256 hex (this surface is
    /// app-facing, unlike the raw-byte push lane).
    MergePr {
        #[serde(default)]
        repo: String,
        number: u64,
        prev_target_oid: String,
        expected_source_oid: String,
        merge_oid: String,
        pack_digest: String,
    },
    /// submit a batched review on a PR: one verdict, an optional body, and
    /// line-anchored diff comments, anchored at `commit_oid` (the source head
    /// the reviewer saw). approvals are advisory — never merge-blocking.
    SubmitReview {
        #[serde(default)]
        repo: String,
        number: u64,
        verdict: ReviewVerdict,
        #[serde(default)]
        body: String,
        commit_oid: String,
        #[serde(default)]
        comments: Vec<ReviewComment>,
    },
}

/// reads over the repo namespace.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForgeQuery {
    /// the canonical head of the `"default"` repo — the single-repo query
    /// (a unit variant: the bare `"head"` string on the wire).
    Head,
    /// the canonical head of a named repo (empty -> `"default"`).
    HeadOf { repo: String },
    /// every repo in the namespace with its committed head, sorted by name.
    ListRepos,
    /// every born branch of a repo, sorted by name.
    ListRefs { repo: String },
    /// every issue/PR of a repo, ascending by number (team-scale: no paging).
    ListItems { repo: String },
    /// one item in full — body, branches, reviews, discussion channel id.
    GetItem { repo: String, number: u64 },
}

/// the git oid hex of a repo's HEAD (a 40-char sha1 oid), or `None` on an unborn
/// repo (no commits yet). a consumer can git-address the exact commit forge
/// holds while the app-hash keeps sha256-strength (the head oid is the root's
/// preimage material).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForgeReply {
    /// a single repo's head hex (the reply to [`ForgeQuery::Head`]/[`ForgeQuery::
    /// HeadOf`]).
    Head(Option<String>),
    /// the whole namespace: one [`RepoHead`] per repo, sorted by name (the reply
    /// to [`ForgeQuery::ListRepos`]).
    Repos(Vec<RepoHead>),
    /// a repo's born branches (the reply to [`ForgeQuery::ListRefs`]).
    Refs(Vec<crate::tracker_iface::RefHead>),
    /// a repo's items (the reply to [`ForgeQuery::ListItems`]).
    Items(Vec<crate::tracker_iface::ItemSummary>),
    /// one full item (the reply to [`ForgeQuery::GetItem`]). boxed: an
    /// ItemDetail dwarfs the other variants.
    Item(Option<Box<crate::tracker_iface::ItemDetail>>),
}

/// one repo's committed head in a [`ForgeReply::Repos`] listing.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RepoHead {
    /// the repo's normalized slug.
    pub name: String,
    /// the repo's committed HEAD oid as hex, or `None` if the repo is unborn.
    pub head: Option<String>,
}

pub fn encode_msg(m: &ForgeMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<ForgeMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &ForgeQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<ForgeQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &ForgeReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<ForgeReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_without_repo_decodes_with_empty_repo() {
        // the exact bytes a single-repo client (and the app's forge-client)
        // sends: no `repo` key. `#[serde(default)]` must fill it with "" so the
        // module can map it to the default repo — this is the defaulting contract.
        let legacy = br#"{"commit":{"path":"a.txt","content":"hi","message":"m"}}"#;
        let msg = decode_msg(legacy).expect("repo-less commit must decode");
        assert_eq!(
            msg,
            ForgeMsg::Commit {
                repo: String::new(),
                path: "a.txt".into(),
                content: "hi".into(),
                message: "m".into(),
            }
        );
    }

    #[test]
    fn push_refs_without_repo_decodes_with_empty_repo() {
        let legacy = br#"{"push_refs":{"updates":[{"ref_name":"main","prev_oid":null,"new_oid":[1,2,3]}],"pack_digest":[4,5]}}"#;
        let msg = decode_msg(legacy).expect("repo-less push_refs must decode");
        assert_eq!(
            msg,
            ForgeMsg::PushRefs {
                repo: String::new(),
                updates: vec![RefUpdate {
                    ref_name: "main".into(),
                    prev_oid: None,
                    new_oid: Some(vec![1, 2, 3]),
                }],
                pack_digest: Some(vec![4, 5]),
            }
        );
    }

    #[test]
    fn bare_head_query_decodes_as_the_unit_variant() {
        assert_eq!(decode_query(br#""head""#).unwrap(), ForgeQuery::Head);
    }

    #[test]
    fn new_query_and_reply_variants_round_trip() {
        for q in [
            ForgeQuery::Head,
            ForgeQuery::HeadOf {
                repo: "docs".into(),
            },
            ForgeQuery::ListRepos,
        ] {
            assert_eq!(decode_query(&encode_query(&q)).unwrap(), q);
        }
        let reply = ForgeReply::Repos(vec![
            RepoHead {
                name: "a".into(),
                head: Some("deadbeef".into()),
            },
            RepoHead {
                name: "b".into(),
                head: None,
            },
        ]);
        assert_eq!(decode_reply(&encode_reply(&reply)).unwrap(), reply);
    }
}
