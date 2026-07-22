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
//! the `repo` field is REQUIRED on every [`ForgeMsg`] variant, but an empty
//! slug is a first-class value: the module normalizes `repo == ""` to the
//! well-known `"default"` repo. a single-repo client sends `repo: ""` and
//! queries `"head"` to keep targeting one canonical repo; the multi-repo
//! surface ([`ForgeQuery::HeadOf`]/[`ForgeQuery::ListRepos`]) is purely
//! additive.

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
/// every variant names its target repo via `repo` (required on the wire). an
/// empty `repo` slug maps to the `"default"` repo (see the module docstring).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForgeMsg {
    Commit {
        /// the target repo slug; empty -> the `"default"` repo.
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
        repo: String,
        updates: Vec<RefUpdate>,
        pack_digest: Option<Vec<u8>>,
    },
    /// open an issue. assigns the repo's next shared number, stores title/body
    /// on the record (authorship is origin-derived), and emits a follow-up
    /// creating the hidden discussion channel `forge:<repo>:<n>` — atomic with
    /// the record.
    OpenIssue {
        repo: String,
        title: String,
        #[serde(default)]
        body: String,
    },
    /// open a pull request from a born `source_branch` onto `target_branch`
    /// (empty -> "dev"). same number space + discussion channel as issues.
    OpenPr {
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
        repo: String,
        number: u64,
        title: Option<String>,
        body: Option<String>,
    },
    /// close (`open: false`) or reopen (`open: true`) an item. merged PRs are
    /// terminal; an unchanged state is a deterministic no-op.
    SetItemState {
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
    /// a pull request's current source-vs-target patch, pinned to the exact
    /// committed branch heads. The node-local object store must contain both
    /// commits and their trees; this query never fetches missing objects.
    PrDiff { repo: String, number: u64 },
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
    /// a bounded, reviewable pull-request patch (the reply to
    /// [`ForgeQuery::PrDiff`]).
    PrDiff(PrDiff),
}

/// Maximum UTF-8 bytes returned in [`PrDiff::patch`]. The limit is fixed by the
/// server rather than caller-controlled so one tool call cannot consume an
/// agent's context.
pub const MAX_PR_DIFF_BYTES: usize = 48 * 1024;
/// Maximum number of changed paths examined for one PR diff.
pub const MAX_PR_DIFF_FILES: usize = 256;
/// Maximum aggregate old-plus-new blob bytes examined for one PR diff.
pub const MAX_PR_DIFF_BLOB_BYTES: usize = 8 * 1024 * 1024;
/// Maximum aggregate bytes of the two commit objects inspected for one PR
/// diff. Headers are checked before libgit2 materializes either commit.
pub const MAX_PR_DIFF_COMMIT_BYTES: usize = 256 * 1024;
/// Maximum old-plus-new tree entries visited while preflighting one PR diff.
pub const MAX_PR_DIFF_TREE_ENTRIES: usize = 4 * 1024;
/// Maximum aggregate bytes of tree objects loaded while preflighting one PR
/// diff. Tree headers are checked before libgit2 materializes the object.
pub const MAX_PR_DIFF_TREE_BYTES: usize = 4 * 1024 * 1024;
/// Maximum recursive tree depth visited while preflighting one PR diff.
pub const MAX_PR_DIFF_TREE_DEPTH: usize = 64;

/// An exact source/target comparison at the committed OIDs named here.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PrDiff {
    pub source_oid: String,
    pub target_oid: String,
    pub files_changed: usize,
    pub additions: usize,
    pub deletions: usize,
    pub patch: String,
    /// True when `patch` is only a prefix of the full unified diff. Statistics
    /// are still complete because over-limit file/blob inputs fail instead of
    /// returning a partial reply.
    pub truncated: bool,
}

/// one repo's committed head in a [`ForgeReply::Repos`] listing.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RepoHead {
    /// the repo's normalized slug.
    pub name: String,
    /// the repo's committed INTEGRATION head (dev, falling back to main) as
    /// hex, or `None` if neither branch is born.
    pub head: Option<String>,
}

pub fn encode_msg(m: &ForgeMsg) -> Vec<u8> {
    sdk::wire::encode(m)
}
pub fn decode_msg(b: &[u8]) -> Result<ForgeMsg, String> {
    sdk::wire::decode(b)
}
pub fn encode_query(q: &ForgeQuery) -> Vec<u8> {
    sdk::wire::encode(q)
}
pub fn decode_query(b: &[u8]) -> Result<ForgeQuery, String> {
    sdk::wire::decode(b)
}
pub fn encode_reply(r: &ForgeReply) -> Vec<u8> {
    sdk::wire::encode(r)
}
pub fn decode_reply(b: &[u8]) -> Result<ForgeReply, String> {
    sdk::wire::decode(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_is_required_on_the_wire() {
        // `repo` is a required field (no `#[serde(default)]`): a message that
        // omits the key is rejected. every live producer emits `repo` — the
        // single-repo ergonomic is `repo: ""`, an explicit empty slug that the
        // module maps to the default repo, not an absent key.
        let no_repo_key = br#"{"commit":{"path":"a.txt","content":"hi","message":"m"}}"#;
        assert!(
            decode_msg(no_repo_key).is_err(),
            "a commit without a repo key must be rejected"
        );
        let no_repo_push = br#"{"push_refs":{"updates":[{"ref_name":"main","prev_oid":null,"new_oid":[1,2,3]}],"pack_digest":[4,5]}}"#;
        assert!(
            decode_msg(no_repo_push).is_err(),
            "a push_refs without a repo key must be rejected"
        );
        // the explicit empty slug still decodes (single-repo ergonomic).
        let empty_repo = br#"{"commit":{"repo":"","path":"a.txt","content":"hi","message":"m"}}"#;
        assert_eq!(
            decode_msg(empty_repo).unwrap(),
            ForgeMsg::Commit {
                repo: String::new(),
                path: "a.txt".into(),
                content: "hi".into(),
                message: "m".into(),
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
