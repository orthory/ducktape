//! the forge compose lane (M1): trigger channel `forge:<repo>:<n>` → a
//! git-native workspace source pinned from COMMITTED forge state.
//!
//! this module owns the three compose rules that make a forge item a SESSION:
//! the per-item work branch (`agent/item-<n>` for issues, the PR's own source
//! branch for PRs — that one rule IS the PR=session feature), the pinned base
//! commit (the work-branch tip when born, else the main tip an issue forks
//! from), and the requested `Pr` sink. everything here reads committed state
//! at compose height (I1): item lookup and refs go through the ctx query lane
//! with local serde MIRRORS of forge's wire types — `forge` stays a DEV-ONLY
//! dependency of runs (vendored libgit2 stays out of the production build),
//! and dev-only conformance tests pin every mirror against the real forge
//! codec so the wire cannot silently drift.

use agent::{AgentRecord, CapRequest};
use sdk::Ctx;
use serde::{Deserialize, Serialize};

use crate::envelope::{self, PortableInputs, WorkspaceSource};
use crate::facets::WireSink;
use crate::sink::ForgeSinkQuery;
use crate::{RunsModule, inject};

/// the branch an issue run forks from and PRs default-target. forge has no
/// per-repo default-branch state — "main" is its constant
/// (`forge::refs::MAIN_BRANCH`, pinned by a dev-only conformance test).
pub(crate) const MAIN_BRANCH: &str = "main";

/// the channel-id prefix of a forge item's hidden discussion channel
/// (`forge::channel_id_for`): `forge:<repo>:<n>`.
const FORGE_CHANNEL_PREFIX: &str = "forge:";

/// parsed coordinates of a forge item channel.
pub(crate) struct ForgeItemRef<'a> {
    pub repo: &'a str,
    pub number: u64,
}

/// detect a forge item channel: `forge:<repo>:<n>` parses to its coordinates;
/// ANYTHING malformed is not a forge channel and the caller composes the
/// duckfs lane as today. the number must be all ASCII digits (no sign, no
/// float forms) and fit u64. the repo segment is taken verbatim — real forge
/// channels are minted by the forge module itself under its reserved `forge:`
/// chat namespace with a validated slug, so a non-slug repo here can only
/// name an item that does not exist (a compose error downstream).
pub(crate) fn parse_forge_channel(channel_id: &str) -> Option<ForgeItemRef<'_>> {
    let rest = channel_id.strip_prefix(FORGE_CHANNEL_PREFIX)?;
    let (repo, number) = rest.rsplit_once(':')?;
    if repo.is_empty() || number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(ForgeItemRef {
        repo,
        number: number.parse().ok()?,
    })
}

// ---- committed tracker/refs mirrors ------------------------------------------
// runs does NOT take a production dependency on the heavy `forge` crate; these
// mirror the exact JSON shapes forge speaks, pinned by the dev-only
// conformance tests below (the ListRefs sink-probe mirror in lib.rs is the
// established pattern).

/// the committed tracker queries mirrored here: `GetItem` (the compose lane's
/// item lookup, also the sink guard's per-PR read) and `ListItems` (the sink's
/// duplicate-PR guard sweep).
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ForgeTrackerQuery<'a> {
    GetItem { repo: &'a str, number: u64 },
    ListItems { repo: &'a str },
}

/// mirror of `forge::ItemKind`.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ForgeItemKind {
    Issue,
    Pr,
}

/// mirror of `forge::ItemState`.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ForgeItemState {
    Open,
    Closed,
    Merged,
}

impl ForgeItemKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ForgeItemKind::Issue => "issue",
            ForgeItemKind::Pr => "pr",
        }
    }
}

impl ForgeItemState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ForgeItemState::Open => "open",
            ForgeItemState::Closed => "closed",
            ForgeItemState::Merged => "merged",
        }
    }
}

/// the slice of `forge::ItemDetail` the compose lane reads (the reply is flat:
/// the summary is `#[serde(flatten)]`ed on the forge side; unknown fields —
/// author, reviews, channel_id, … — are ignored on decode).
#[derive(Deserialize, Debug)]
pub(crate) struct ForgeItem {
    pub number: u64,
    pub kind: ForgeItemKind,
    pub title: String,
    pub state: ForgeItemState,
    pub body: String,
    /// PR-only: the source branch short name.
    pub source_branch: Option<String>,
    /// PR-only: the target branch short name.
    pub target_branch: Option<String>,
}

/// mirror of `forge::ForgeReply::Item(Option<Box<ItemDetail>>)`.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ItemReplyMirror {
    Item(Option<ForgeItem>),
}

/// decode a `GetItem` reply: `Ok(None)` is a missing item, `Err` an
/// unexpected reply shape.
fn decode_item_reply(bytes: &[u8]) -> Result<Option<ForgeItem>, String> {
    let ItemReplyMirror::Item(item) = serde_json::from_slice(bytes)
        .map_err(|e| format!("undecodable forge item reply: {e}"))?;
    Ok(item)
}

/// the slice of `forge::ItemSummary` the duplicate-PR guard reads from a
/// `ListItems` reply (unknown fields — title, author, timestamps — are
/// ignored on decode).
#[derive(Deserialize, Debug)]
pub(crate) struct ForgeItemSummary {
    pub number: u64,
    pub kind: ForgeItemKind,
    pub state: ForgeItemState,
}

/// mirror of `forge::ForgeReply::Items(Vec<ItemSummary>)`.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ItemsReplyMirror {
    Items(Vec<ForgeItemSummary>),
}

/// decode a `ListItems` reply into item summaries (ascending by number — the
/// tracker's listing order).
fn decode_items_reply(bytes: &[u8]) -> Result<Vec<ForgeItemSummary>, String> {
    let ItemsReplyMirror::Items(items) = serde_json::from_slice(bytes)
        .map_err(|e| format!("undecodable forge items reply: {e}"))?;
    Ok(items)
}

/// one born branch in a `ListRefs` reply — mirror of `forge::RefHead`.
#[derive(Deserialize, Debug)]
pub(crate) struct ForgeRefHead {
    /// the branch SHORT name ("main", "feature/x").
    pub name: String,
    /// the branch tip as 40-char sha1 hex.
    pub head: String,
}

/// mirror of `forge::ForgeReply::Refs(Vec<RefHead>)`.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum RefsReplyMirror {
    Refs(Vec<ForgeRefHead>),
}

/// decode a `ListRefs` reply into born branches with their tips.
fn decode_refs_reply(bytes: &[u8]) -> Result<Vec<ForgeRefHead>, String> {
    let RefsReplyMirror::Refs(refs) = serde_json::from_slice(bytes)
        .map_err(|e| format!("undecodable forge refs reply: {e}"))?;
    Ok(refs)
}

// ---- the compose lane ----------------------------------------------------------

impl RunsModule {
    /// the portable inputs of a forge-channel run: the pinned forge workspace
    /// source, the requested PR sink, the deterministic item context, and the
    /// same skill resolution as the duckfs lane. every read is COMMITTED
    /// state at compose height (I1); any failure is a deterministic reason
    /// that lands in the EXISTING compose-failure lanes (engagement skip +
    /// breadcrumb, RequestRun rejection).
    pub(crate) async fn forge_portable_inputs(
        &self,
        ctx: &dyn Ctx,
        agent: &AgentRecord,
        item_ref: &ForgeItemRef<'_>,
    ) -> Result<PortableInputs, String> {
        let repo = item_ref.repo;
        // 1. the cap gate FIRST — before any tracker read.
        if !agent.permits(&CapRequest::ForgeRead(repo)) {
            return Err(format!(
                "agent {} lacks forge_read for {repo}",
                agent.agent_id
            ));
        }
        let Some(forge) = self.forge.clone() else {
            return Err("no forge module is wired for a forge-channel run".into());
        };
        // 2. the committed tracker item.
        let item = self
            .forge_item(ctx, &forge, repo, item_ref.number)
            .await?
            .ok_or_else(|| format!("no forge item {repo}#{}", item_ref.number))?;
        // 3. the work branch — per ITEM, not per run (session identity):
        //    an issue works `agent/item-<n>`; a PR works ITS OWN source
        //    branch, so the session's pushes update the open PR in place.
        let branch = match item.kind {
            ForgeItemKind::Issue => format!("agent/item-{}", item_ref.number),
            ForgeItemKind::Pr => item
                .source_branch
                .clone()
                .filter(|b| !b.is_empty())
                .ok_or_else(|| {
                    format!("forge pr {repo}#{} has no source branch", item_ref.number)
                })?,
        };
        // 4. the pinned base commit + branch_born, from COMMITTED refs.
        let refs = self.forge_refs(ctx, &forge, repo).await?;
        let tip = |name: &str| {
            refs.iter()
                .find(|r| r.name == name)
                .map(|r| r.head.clone())
        };
        let (commit, branch_born) = match tip(&branch) {
            // the branch is born: the session continues — fork ITS tip.
            Some(tip) => (tip, true),
            None => match item.kind {
                // first run for an issue: fork the main tip; the provisioner
                // creates the branch (zero-oid CAS base).
                ForgeItemKind::Issue => (
                    tip(MAIN_BRANCH).ok_or_else(|| {
                        format!("repo {repo} has no {MAIN_BRANCH} branch to fork")
                    })?,
                    false,
                ),
                // a PR's work branch IS its source branch, born by
                // construction while the PR exists; a deleted source is a
                // real compose failure, never a silent re-create.
                ForgeItemKind::Pr => {
                    return Err(format!(
                        "forge pr {repo}#{} source branch {branch} is not born",
                        item_ref.number
                    ));
                }
            },
        };
        // 5. the requested sink: a PR of the work branch onto the item's
        //    target (issues target main). title/body stay empty — delivery
        //    derives them from the message facet.
        let target_branch = match item.kind {
            ForgeItemKind::Pr => item
                .target_branch
                .clone()
                .filter(|b| !b.is_empty())
                .unwrap_or_else(|| MAIN_BRANCH.to_string()),
            ForgeItemKind::Issue => MAIN_BRANCH.to_string(),
        };
        let sink = WireSink::Pr {
            repo: repo.to_string(),
            source_branch: branch.clone(),
            target_branch,
            title: String::new(),
            body: String::new(),
        };
        // 6. the deterministic item-context section (byte-capped in inject).
        let context = inject::render_item_context(repo, &item, &branch);
        // skills are duckfs subtrees in every lane: resolve them against the
        // committed duckfs head exactly as the duckfs lane does (W2).
        let head = match self.files.clone() {
            Some(files) => self.duckfs_head(ctx, &files).await?,
            None => None,
        };
        Ok(PortableInputs {
            workspace: WorkspaceSource::Forge {
                repo: repo.to_string(),
                commit,
                branch,
                branch_born,
            },
            skills: envelope::resolve_skills(agent, &head),
            sink,
            context: Some(context),
        })
    }

    /// one committed tracker item, or `None` when it does not exist.
    pub(crate) async fn forge_item(
        &self,
        ctx: &dyn Ctx,
        forge: &str,
        repo: &str,
        number: u64,
    ) -> Result<Option<ForgeItem>, String> {
        let reply = ctx
            .query(
                forge,
                &serde_json::to_vec(&ForgeTrackerQuery::GetItem { repo, number })
                    .expect("query serializes"),
            )
            .await
            .map_err(|e| format!("forge item lookup failed: {e}"))?;
        decode_item_reply(&reply)
    }

    /// a repo's committed tracker items as summaries, ascending by number
    /// (a committed-only read — validator-uniform, like the refs listing).
    pub(crate) async fn forge_item_summaries(
        &self,
        ctx: &dyn Ctx,
        forge: &str,
        repo: &str,
    ) -> Result<Vec<ForgeItemSummary>, String> {
        let reply = ctx
            .query(
                forge,
                &serde_json::to_vec(&ForgeTrackerQuery::ListItems { repo })
                    .expect("query serializes"),
            )
            .await
            .map_err(|e| format!("forge items lookup failed: {e}"))?;
        decode_items_reply(&reply)
    }

    /// a repo's committed born branches with their tips (`ListRefs` is a
    /// committed-only read — never staged/read-your-writes, so the pin is
    /// validator-uniform).
    async fn forge_refs(
        &self,
        ctx: &dyn Ctx,
        forge: &str,
        repo: &str,
    ) -> Result<Vec<ForgeRefHead>, String> {
        let reply = ctx
            .query(
                forge,
                &serde_json::to_vec(&ForgeSinkQuery::ListRefs { repo }).expect("query serializes"),
            )
            .await
            .map_err(|e| format!("forge refs lookup failed: {e}"))?;
        decode_refs_reply(&reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- channel parsing ------------------------------------------------------

    #[test]
    fn forge_channel_parse_accepts_canonical_item_channels() {
        let item = parse_forge_channel("forge:app:7").expect("canonical id parses");
        assert_eq!(item.repo, "app");
        assert_eq!(item.number, 7);

        let item = parse_forge_channel("forge:my-repo.v2:12345").expect("slug chars parse");
        assert_eq!(item.repo, "my-repo.v2");
        assert_eq!(item.number, 12345);
    }

    #[test]
    fn forge_channel_parse_rejects_everything_else_as_not_a_forge_channel() {
        for id in [
            "general",
            "forge",
            "forge:",
            "forge:app",
            "forge:app:",
            "forge::7",
            "forge:app:x7",
            "forge:app:7x",
            "forge:app:-1",
            "forge:app:+7",
            "forge:app:7.5",
            // u64 overflow.
            "forge:app:18446744073709551616",
        ] {
            assert!(
                parse_forge_channel(id).is_none(),
                "{id:?} must NOT parse as a forge channel (duckfs as today)"
            );
        }
    }

    // ---- mirror conformance (dev-only, against the real forge codec) ----------

    #[test]
    fn main_branch_const_matches_forge() {
        assert_eq!(MAIN_BRANCH, forge::refs::MAIN_BRANCH);
    }

    #[test]
    fn get_item_query_mirror_matches_forge_decode_query() {
        let bytes = serde_json::to_vec(&ForgeTrackerQuery::GetItem {
            repo: "app",
            number: 7,
        })
        .unwrap();
        assert_eq!(
            forge::decode_query(&bytes).unwrap(),
            forge::ForgeQuery::GetItem {
                repo: "app".into(),
                number: 7,
            }
        );
    }

    #[test]
    fn list_refs_query_mirror_matches_forge_decode_query() {
        let bytes =
            serde_json::to_vec(&crate::sink::ForgeSinkQuery::ListRefs { repo: "app" }).unwrap();
        assert_eq!(
            forge::decode_query(&bytes).unwrap(),
            forge::ForgeQuery::ListRefs { repo: "app".into() }
        );
    }

    #[test]
    fn list_items_query_mirror_matches_forge_decode_query() {
        let bytes = serde_json::to_vec(&ForgeTrackerQuery::ListItems { repo: "app" }).unwrap();
        assert_eq!(
            forge::decode_query(&bytes).unwrap(),
            forge::ForgeQuery::ListItems { repo: "app".into() }
        );
    }

    #[test]
    fn items_reply_mirror_decodes_the_real_forge_reply() {
        let summary = |number, kind, state| forge::ItemSummary {
            number,
            kind,
            title: "t".into(),
            state,
            author: chat::AuthorRef::User(vec![1; 32]),
            created_at: 1,
            updated_at: 2,
        };
        let bytes = forge::encode_reply(&forge::ForgeReply::Items(vec![
            summary(3, forge::ItemKind::Issue, forge::ItemState::Open),
            summary(4, forge::ItemKind::Pr, forge::ItemState::Merged),
            summary(5, forge::ItemKind::Pr, forge::ItemState::Closed),
        ]));
        let items = decode_items_reply(&bytes).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].number, 3);
        assert_eq!(items[0].kind, ForgeItemKind::Issue);
        assert_eq!(items[0].state, ForgeItemState::Open);
        assert_eq!(items[1].number, 4);
        assert_eq!(items[1].kind, ForgeItemKind::Pr);
        assert_eq!(items[1].state, ForgeItemState::Merged);
        assert_eq!(items[2].state, ForgeItemState::Closed);
    }

    fn detail(kind: forge::ItemKind, branches: Option<(&str, &str)>) -> forge::ItemDetail {
        forge::ItemDetail {
            summary: forge::ItemSummary {
                number: 7,
                kind,
                title: "Fix the flaky gate".into(),
                state: forge::ItemState::Open,
                author: chat::AuthorRef::User(vec![1; 32]),
                created_at: 1,
                updated_at: 2,
            },
            body: "repro:\n- run it twice".into(),
            channel_id: "forge:app:7".into(),
            source_branch: branches.map(|(s, _)| s.to_string()),
            target_branch: branches.map(|(_, t)| t.to_string()),
            merge_oid: None,
            reviews: Vec::new(),
        }
    }

    #[test]
    fn item_reply_mirror_decodes_the_real_forge_reply() {
        // an issue: no branches.
        let bytes = forge::encode_reply(&forge::ForgeReply::Item(Some(Box::new(detail(
            forge::ItemKind::Issue,
            None,
        )))));
        let item = decode_item_reply(&bytes).unwrap().expect("item present");
        assert_eq!(item.number, 7);
        assert_eq!(item.kind, ForgeItemKind::Issue);
        assert_eq!(item.state, ForgeItemState::Open);
        assert_eq!(item.title, "Fix the flaky gate");
        assert_eq!(item.body, "repro:\n- run it twice");
        assert_eq!(item.source_branch, None);
        assert_eq!(item.target_branch, None);

        // a PR: branches present.
        let bytes = forge::encode_reply(&forge::ForgeReply::Item(Some(Box::new(detail(
            forge::ItemKind::Pr,
            Some(("feature/x", "dev")),
        )))));
        let item = decode_item_reply(&bytes).unwrap().expect("item present");
        assert_eq!(item.kind, ForgeItemKind::Pr);
        assert_eq!(item.source_branch.as_deref(), Some("feature/x"));
        assert_eq!(item.target_branch.as_deref(), Some("dev"));

        // a missing item decodes as None, not an error.
        let bytes = forge::encode_reply(&forge::ForgeReply::Item(None));
        assert!(decode_item_reply(&bytes).unwrap().is_none());
    }

    #[test]
    fn item_state_mirror_decodes_every_forge_state() {
        for (state, expected) in [
            (forge::ItemState::Open, ForgeItemState::Open),
            (forge::ItemState::Closed, ForgeItemState::Closed),
            (forge::ItemState::Merged, ForgeItemState::Merged),
        ] {
            let mut d = detail(forge::ItemKind::Issue, None);
            d.summary.state = state;
            let bytes = forge::encode_reply(&forge::ForgeReply::Item(Some(Box::new(d))));
            assert_eq!(decode_item_reply(&bytes).unwrap().unwrap().state, expected);
        }
    }

    #[test]
    fn refs_reply_mirror_decodes_names_and_tips() {
        let bytes = forge::encode_reply(&forge::ForgeReply::Refs(vec![
            forge::RefHead {
                name: "agent/item-7".into(),
                head: "ab".repeat(20),
            },
            forge::RefHead {
                name: "main".into(),
                head: "cd".repeat(20),
            },
        ]));
        let refs = decode_refs_reply(&bytes).unwrap();
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].name, "agent/item-7");
        assert_eq!(refs[0].head, "ab".repeat(20));
        assert_eq!(refs[1].name, "main");
        assert_eq!(refs[1].head, "cd".repeat(20));
    }
}
