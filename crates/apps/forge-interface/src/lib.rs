//! the forge module's public wire surface — types only.
//!
//! forge is a git-backed module: its state is a NAMED NAMESPACE of git repos,
//! addressed by a repo slug (`[a-z0-9._-]`, 1..=64 bytes). its `root()` is a
//! canonical sorted hash over the committed HEAD oid of every repo that has a
//! head. writes go via [`ForgeMsg`] (a file put + commit, or a git push); reads
//! via [`ForgeQuery`] -> [`ForgeReply`], returning HEAD oids as hex.
//!
//! ## back-compat: the default repo
//!
//! the `repo` field on [`ForgeMsg::Commit`]/[`ForgeMsg::Push`] carries
//! `#[serde(default)]`, so a wire message that omits it deserializes with
//! `repo == ""`; the module normalizes an empty repo to the well-known
//! `"default"` repo. an old client that sends `{"Commit":{path,content,message}}`
//! and queries `"Head"` therefore keeps targeting one canonical repo with no
//! change — the multi-repo surface ([`ForgeQuery::HeadOf`]/[`ForgeQuery::
//! ListRepos`]) is purely additive.

use serde::{Deserialize, Serialize};

/// a write intent at forge: either the file-by-file [`ForgeMsg::Commit`] (forge
/// builds the commit object itself) or [`ForgeMsg::Push`] — a git-faithful ref
/// update that adopts a client's REAL commit history by oid, with the objects
/// carried out-of-band in a node-local packfile (never in consensus).
///
/// both variants name their target repo via `repo`. the field is
/// `#[serde(default)]`, so an omitted/empty `repo` deserializes to `""` and the
/// module maps it to the `"default"` repo (see the module docstring) — the
/// legacy single-repo wire is preserved byte-for-byte.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ForgeMsg {
    Commit {
        /// the target repo slug; empty/absent -> the `"default"` repo.
        #[serde(default)]
        repo: String,
        path: String,
        content: String,
        message: String,
    },
    /// a git ref update over consensus. the ONLY consensus effect is a
    /// compare-and-swap on the target repo's committed HEAD: that repo's current
    /// HEAD must equal `prev_oid`, and on match its HEAD becomes `new_oid` (so
    /// the composed `root()` moves on EVERY validator, pack-holder or not). the
    /// git objects themselves are node-local — fetched from the files blob store
    /// by `pack_digest` and installed lazily — and NEVER influence root/accept.
    Push {
        /// the target repo slug; empty/absent -> the `"default"` repo.
        #[serde(default)]
        repo: String,
        /// the CAS guard: the repo's committed HEAD must equal this or the push
        /// is rejected (non-fast-forward). `None` == the repo is unborn (pushing
        /// to an empty remote). 20 raw sha1 bytes when `Some`.
        prev_oid: Option<Vec<u8>>,
        /// the new committed HEAD after the push. 20 raw sha1 bytes.
        new_oid: Vec<u8>,
        /// sha256 digest of the packfile (full object closure of `new_oid`) in
        /// the node's files blob store. objects are NODE-LOCAL, never consensus
        /// state; this 32-byte locator has ZERO effect on root/accept-reject.
        pack_digest: Vec<u8>,
    },
}

/// reads over the repo namespace.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ForgeQuery {
    /// the canonical head of the `"default"` repo — the legacy single-repo query
    /// (kept as a unit variant so an old `"Head"` message still works).
    Head,
    /// the canonical head of a named repo (empty -> `"default"`).
    HeadOf { repo: String },
    /// every repo in the namespace with its committed head, sorted by name.
    ListRepos,
}

/// the git oid hex of a repo's HEAD (a 40-char sha1 oid), or `None` on an unborn
/// repo (no commits yet). a consumer can git-address the exact commit forge
/// holds while the app-hash keeps sha256-strength (the head oid is the root's
/// preimage material).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ForgeReply {
    /// a single repo's head hex (the reply to [`ForgeQuery::Head`]/[`ForgeQuery::
    /// HeadOf`]).
    Head(Option<String>),
    /// the whole namespace: one [`RepoHead`] per repo, sorted by name (the reply
    /// to [`ForgeQuery::ListRepos`]).
    Repos(Vec<RepoHead>),
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
    fn legacy_commit_without_repo_decodes_with_empty_repo() {
        // the exact bytes an old client (and the app's forge-client) sends: no
        // `repo` key. `#[serde(default)]` must fill it with "" so the module can
        // map it to the default repo — this is the whole back-compat contract.
        let legacy = br#"{"Commit":{"path":"a.txt","content":"hi","message":"m"}}"#;
        let msg = decode_msg(legacy).expect("legacy Commit must still decode");
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
    fn legacy_push_without_repo_decodes_with_empty_repo() {
        let legacy = br#"{"Push":{"prev_oid":null,"new_oid":[1,2,3],"pack_digest":[4,5]}}"#;
        let msg = decode_msg(legacy).expect("legacy Push must still decode");
        assert_eq!(
            msg,
            ForgeMsg::Push {
                repo: String::new(),
                prev_oid: None,
                new_oid: vec![1, 2, 3],
                pack_digest: vec![4, 5],
            }
        );
    }

    #[test]
    fn legacy_head_query_still_decodes_as_the_unit_variant() {
        assert_eq!(decode_query(br#""Head""#).unwrap(), ForgeQuery::Head);
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
