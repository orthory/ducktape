//! the delivery sink (O1/O2): applying a run's requested output sink at the
//! result intake — today the forge PR sink.
//!
//! everything here runs inside the delivery block and follows the NO-FAIL
//! rule (R4): every missing precondition — malformed sink, unwired forge,
//! missing agent, missing `ForgePush` cap, unborn branch, unreadable tracker
//! state — degrades to a [`RunsModule::note`] breadcrumb and never aborts.
//! reads are COMMITTED forge state via ctx queries with local serde MIRRORS
//! of forge's wire types (`forge` stays a DEV-ONLY dependency; conformance
//! tests pin every mirror against the real forge codec).

use agent::{CapRequest, ReplyBlock};
use saga::{
    SagaQuery, SagaReply, decode_reply as saga_decode_reply, encode_query as saga_encode_query,
};
use sdk::{Ctx, Msg};
use serde::Serialize;

use crate::facets::{WireSink, WorkspaceReceipt};
use crate::forge_source::{ForgeItemKind, ForgeItemState};
use crate::response::REPLY_KIND_CODE;
use crate::{RunsModule, truncate_on_boundary};

/// the fallback PR-title clamp: the first line of the message facet, at most
/// this many CHARS (char-boundary safe, never a byte slice).
const PR_TITLE_MAX_CHARS: usize = 100;

/// mirror of `forge::MAX_TITLE_BYTES` (conformance-pinned): an OpenPr whose
/// title exceeds it would REJECT — and a rejected follow-up aborts the
/// delivery block, so the derivation must clamp below it (100 4-byte chars
/// would be 400 bytes).
const FORGE_TITLE_BYTE_CAP: usize = 256;

/// mirror of `forge::MAX_BODY_BYTES` (conformance-pinned) — same no-abort
/// reasoning as the title cap. unreachable under the 32 KiB reply-blocks cap;
/// kept as a deterministic guard.
const FORGE_BODY_BYTE_CAP: usize = 64 * 1024;

/// the saga id the dispatch module derives for one of OUR dispatches — a
/// MIRROR of dispatch's private `saga_id_for(dispatch_key(receiver, id))`
/// (`'\u{1f}'`-joined, "dispatch"-prefixed), pinned by a conformance test that
/// drives the real dispatch module. the executing-node lookup needs it to
/// read the run's Done saga record.
pub(crate) fn saga_id_for_dispatch(receiver: &str, dispatch_id: &str) -> String {
    format!("dispatch\u{1f}{receiver}\u{1f}{dispatch_id}")
}

/// render the validated response's reply blocks — the message facet exactly as
/// chat receives it — into the deterministic prose the PR body carries:
/// paragraphs verbatim, code blocks fenced, blocks joined by one blank line.
/// normalization already guarantees non-empty trimmed texts.
pub(crate) fn message_facet_text(blocks: &[ReplyBlock]) -> String {
    blocks
        .iter()
        .map(|block| match block.kind.as_str() {
            REPLY_KIND_CODE => format!(
                "```{}\n{}\n```",
                block.lang.as_deref().unwrap_or(""),
                block.text
            ),
            _ => block.text.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// PR title: verified bound Forge metadata is authoritative and already obeys
/// Forge's title cap. Only when that metadata is unavailable does the first
/// non-blank response line become a bounded fallback. A blank facet finally
/// falls back to `agent run <run_id>` because Forge rejects empty titles.
fn derive_pr_title(item_title: Option<&str>, message: &str, run_id: &str) -> String {
    if let Some(title) = item_title.filter(|title| !title.trim().is_empty()) {
        return truncate_on_boundary(title, FORGE_TITLE_BYTE_CAP, "");
    }
    let line: String = message
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("agent run {run_id}"))
        .chars()
        .take(PR_TITLE_MAX_CHARS)
        .collect();
    truncate_on_boundary(&line, FORGE_TITLE_BYTE_CAP, "")
}

/// the receipt's output as one honest line: `branch@oid` when a push landed,
/// otherwise an explicit `none` naming why nothing moved.
fn output_ref_line(receipt: &WorkspaceReceipt) -> String {
    match (&receipt.branch, &receipt.output_commit) {
        (Some(branch), Some(oid)) => format!("{branch}@{oid}"),
        _ if receipt.no_changes => "none (no changes this run)".into(),
        _ if receipt.commit_error.is_some() => "none (workspace commit failed)".into(),
        _ => "none".into(),
    }
}

/// PR body: the full message facet plus the receipt breadcrumb block — run
/// id, output_ref (`branch@output_commit`), executing node. deterministic,
/// clamped inside forge's body byte cap (truncating the MESSAGE, never the
/// breadcrumb).
fn derive_pr_body(message: &str, run_id: &str, receipt: &WorkspaceReceipt, node: &str) -> String {
    let crumb = format!(
        "---\nrun: {run_id}\noutput: {}\nnode: {node}",
        output_ref_line(receipt)
    );
    if message.trim().is_empty() {
        return crumb;
    }
    let body = format!("{message}\n\n{crumb}");
    if body.len() <= FORGE_BODY_BYTE_CAP {
        return body;
    }
    // unreachable while the reply-blocks cap holds; deterministic degrade —
    // truncate the MESSAGE, never the breadcrumb.
    let budget = FORGE_BODY_BYTE_CAP.saturating_sub(crumb.len() + 2);
    format!("{}\n\n{crumb}", truncate_on_boundary(message, budget, "…"))
}

// ---- forge sink wire (local mirrors) -----------------------------------------
// runs does NOT take a production dependency on the heavy `forge` crate (it
// pulls vendored libgit2). instead it mirrors the exact JSON shape forge decodes
// for the sink op it emits, and a dev-only conformance test pins the mirror
// against `forge::decode_msg` so the wire can't silently drift.

/// the exact `ForgeMsg::OpenPr` JSON the forge module decodes. only the PR
/// sink is emitted, so only `OpenPr` is mirrored.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ForgeSinkMsg<'a> {
    OpenPr {
        repo: &'a str,
        title: &'a str,
        body: &'a str,
        source_branch: &'a str,
        target_branch: &'a str,
    },
}

fn forge_open_pr_bytes(repo: &str, title: &str, body: &str, src: &str, tgt: &str) -> Vec<u8> {
    serde_json::to_vec(&ForgeSinkMsg::OpenPr {
        repo,
        title,
        body,
        source_branch: src,
        target_branch: tgt,
    })
    .expect("forge sink msg serializes")
}

/// the `ForgeQuery::ListRefs` mirror the branch-born probe encodes.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ForgeSinkQuery<'a> {
    ListRefs { repo: &'a str },
}

impl RunsModule {
    /// apply the O1/O2 sink. Chain is a breadcrumb/no-op in v1 (durable
    /// output_ref chaining is future work — the receipt already carries the
    /// output_ref for a downstream consumer). Pr emits a forge `OpenPr` gated on
    /// the agent's D3 `ForgePush` cap (Phase 4's `permits`, NOT a KNOWN_ACTIONS
    /// grant), committed-state branch-born probes for BOTH the source and the
    /// target (the no-fail rule: an OpenPr for an unborn branch — either end —
    /// or with source == target would abort the block), and the duplicate-PR
    /// guard (an OPEN PR already sourcing this branch was UPDATED by the push
    /// — skip with a breadcrumb). Both paths require a host-observed commit
    /// pushed to this sink's source branch; prose and a stale ref are never a
    /// publication signal. the OpenPr title comes from the verified
    /// bound Forge item; only an unavailable item falls back to `message`.
    /// The body carries the full rendered message facet and `receipt` — the
    /// wire sink's echoed empty title/body are IGNORED. `executing_node` is
    /// the caller-computed saga attribution ([`Self::executing_node`]) the PR
    /// body breadcrumb names.
    /// any missing precondition degrades to a breadcrumb — the sink NEVER
    /// aborts the delivery block.
    ///
    /// returns the PR number this sink touched — the guard-found open PR the
    /// push updated, or the number the emitted `OpenPr` gets (the tracker
    /// numbers items sequentially: committed max + 1) — for the delivered-runs
    /// ring. `None` when no PR was involved.
    #[allow(clippy::too_many_arguments, reason = "delivery-scoped internal seam")]
    pub(crate) async fn emit_sink(
        &self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        entry: &crate::PendingState,
        sink: &WireSink,
        message: &str,
        receipt: &WorkspaceReceipt,
        executing_node: &str,
    ) -> Option<u64> {
        match sink {
            WireSink::Chain => None,
            WireSink::Pr {
                repo,
                source_branch,
                target_branch,
                // delivery derivation is authoritative (§6): the requested
                // sink composes these empty, and even a non-empty echo loses.
                title: _,
                body: _,
            } => {
                // malformed pr sinks degrade to a breadcrumb.
                if repo.is_empty() || source_branch.is_empty() || target_branch.is_empty() {
                    self.note(
                        ctx,
                        format!(
                            "run {run_id} pr sink skipped: incomplete pr sink (repo/source_branch/target_branch required)"
                        ),
                    );
                    return None;
                }
                // forge rejects an OpenPr with identical branches — malformed,
                // same degrade class as the incomplete sink above.
                if source_branch == target_branch {
                    self.note(
                        ctx,
                        format!(
                            "run {run_id} pr sink skipped: source and target are the same branch"
                        ),
                    );
                    return None;
                }
                let Some(forge) = self.forge.clone() else {
                    self.note(
                        ctx,
                        format!("run {run_id} pr sink skipped: no forge module wired"),
                    );
                    return None;
                };
                let agent = match self.agent_for_run(&*ctx, entry).await {
                    Ok(Some(a)) => a,
                    _ => {
                        self.note(
                            ctx,
                            format!("run {run_id} pr sink skipped: agent not registered"),
                        );
                        return None;
                    }
                };
                if !agent.permits(&CapRequest::ForgePush(repo.as_str())) {
                    self.note(
                        ctx,
                        format!("run {run_id} pr sink skipped: agent lacks forge_push for {repo}"),
                    );
                    return None;
                }
                // The workspace receipt is the host-authored publication
                // boundary. A requested PR sink, response prose, and even a
                // pre-existing source ref do not prove this run moved the ref.
                // Gate BEFORE the duplicate-PR probe so no-change/review-only
                // runs cannot be misreported as having updated an existing PR.
                let has_publishable_commit = matches!(
                    (&receipt.branch, &receipt.output_commit),
                    (Some(branch), Some(oid))
                        if receipt.source_prefix == format!("forge:{repo}")
                            && branch == source_branch
                            && !receipt.no_changes
                            && receipt.commit_error.is_none()
                            && oid.len() == 40
                            && oid.bytes().all(|b| b.is_ascii_hexdigit())
                );
                if !has_publishable_commit {
                    self.note(
                        ctx,
                        format!(
                            "run {run_id} pr sink skipped: no publishable workspace commit for source branch"
                        ),
                    );
                    return None;
                }
                match self.forge_branch_born(&*ctx, &forge, repo, source_branch).await {
                    Ok(true) => {}
                    Ok(false) => {
                        self.note(
                            ctx,
                            format!("run {run_id} pr sink skipped: source branch not present"),
                        );
                        return None;
                    }
                    Err(why) => {
                        self.note(ctx, format!("run {run_id} pr sink skipped: {why}"));
                        return None;
                    }
                }
                // A receipt can arrive after its anchor was closed or merged.
                // Re-read committed tracker state at the publication boundary
                // and keep that late result in chat/history only.
                let item_title = match crate::forge_source::parse_forge_channel(&entry.channel_id) {
                    Some(item_ref) if item_ref.repo == repo => {
                        match self.forge_item(&*ctx, &forge, repo, item_ref.number).await {
                            Ok(Some(item)) if item.state == ForgeItemState::Open => {
                                Some(item.title)
                            }
                            Ok(Some(item)) => {
                                self.note(
                                    ctx,
                                    format!(
                                        "run {run_id} pr sink skipped: bound Forge item is {}",
                                        item.state.as_str()
                                    ),
                                );
                                return None;
                            }
                            Ok(None) => {
                                self.note(
                                    ctx,
                                    format!(
                                        "run {run_id} pr sink: bound Forge item unavailable; using response-title fallback"
                                    ),
                                );
                                None
                            }
                            Err(why) => {
                                self.note(
                                    ctx,
                                    format!(
                                        "run {run_id} pr sink: bound Forge item unavailable ({why}); using response-title fallback"
                                    ),
                                );
                                None
                            }
                        }
                    }
                    _ => None,
                };
                // the duplicate-PR guard: an OPEN PR already sourcing this
                // branch means the session's push WAS the feedback — never a
                // second PR.
                let next_number = match self
                    .forge_pr_probe(&*ctx, &forge, repo, source_branch)
                    .await
                {
                    Ok((Some(number), _)) => {
                        self.note(ctx, format!("run {run_id} pr sink: updated PR #{number}"));
                        return Some(number);
                    }
                    Ok((None, next_number)) => next_number,
                    Err(why) => {
                        // an unreadable tracker must not risk a duplicate PR.
                        self.note(ctx, format!("run {run_id} pr sink skipped: {why}"));
                        return None;
                    }
                };
                // forge ALSO rejects an OpenPr whose TARGET branch is unborn
                // in committed refs — and a rejected follow-up aborts the
                // whole delivery block (R4), deterministically, on every
                // retry. checked AFTER the duplicate-PR guard on purpose: an
                // already-open PR emits nothing (no abort risk) and its
                // "updated PR #n" breadcrumb stays honest even when the
                // target was since deleted. born-in-committed-refs also
                // implies the name normalizes (forge validates on push).
                match self.forge_branch_born(&*ctx, &forge, repo, target_branch).await {
                    Ok(true) => {}
                    Ok(false) => {
                        self.note(
                            ctx,
                            format!(
                                "run {run_id} pr sink skipped: target branch {target_branch} not born"
                            ),
                        );
                        return None;
                    }
                    Err(why) => {
                        self.note(ctx, format!("run {run_id} pr sink skipped: {why}"));
                        return None;
                    }
                }
                let title = derive_pr_title(item_title.as_deref(), message, run_id);
                let body = derive_pr_body(message, run_id, receipt, executing_node);
                ctx.emit_msg(Msg {
                    target: forge,
                    payload: forge_open_pr_bytes(repo, &title, &body, source_branch, target_branch),
                });
                Some(next_number)
            }
        }
    }

    /// deterministic committed-state probe: is `branch` a born ref of `repo`?
    /// reads COMMITTED forge state via a query (never node-local pending), so it
    /// is uniform across validators. decoded via `serde_json::Value` to avoid a
    /// production dependency on the forge crate.
    async fn forge_branch_born(
        &self,
        ctx: &dyn Ctx,
        forge: &str,
        repo: &str,
        branch: &str,
    ) -> Result<bool, String> {
        let reply = ctx
            .query(
                forge,
                &serde_json::to_vec(&ForgeSinkQuery::ListRefs { repo }).expect("query serializes"),
            )
            .await
            .map_err(|e| format!("forge refs lookup failed: {e}"))?;
        let value: serde_json::Value =
            serde_json::from_slice(&reply).map_err(|e| format!("undecodable forge reply: {e}"))?;
        let Some(refs) = value.get("refs").and_then(|r| r.as_array()) else {
            return Err("unexpected forge reply for a refs listing".into());
        };
        Ok(refs
            .iter()
            .any(|r| r.get("name").and_then(|n| n.as_str()) == Some(branch)))
    }

    /// the duplicate-PR guard's read, plus the tracker's NEXT item number:
    /// `(open_pr, next_number)`. `open_pr` is the lowest-numbered OPEN PR whose
    /// source branch is `source_branch`, from COMMITTED tracker state
    /// (summaries via the ListItems mirror, then one GetItem per open PR —
    /// `ItemSummary` carries no branches); deterministic: the listing is
    /// ascending by number, first match wins. `next_number` is the number a
    /// fresh `OpenPr` gets — forge numbers items sequentially per repo, so it
    /// is the committed max + 1.
    async fn forge_pr_probe(
        &self,
        ctx: &dyn Ctx,
        forge: &str,
        repo: &str,
        source_branch: &str,
    ) -> Result<(Option<u64>, u64), String> {
        let summaries = self.forge_item_summaries(ctx, forge, repo).await?;
        let next_number = summaries.iter().map(|s| s.number).max().unwrap_or(0) + 1;
        for summary in summaries {
            if summary.kind != ForgeItemKind::Pr || summary.state != ForgeItemState::Open {
                continue;
            }
            let Some(item) = self.forge_item(ctx, forge, repo, summary.number).await? else {
                continue;
            };
            if item.source_branch.as_deref() == Some(source_branch) {
                return Ok((Some(summary.number), next_number));
            }
        }
        Ok((None, next_number))
    }

    /// the run's durable executor attribution: the `assignee` on its DONE saga
    /// record (the winning attempt's lease holder — the node whose
    /// `OracleResult` settled the run), rendered as lowercase key hex. the
    /// saga id is derived by [`saga_id_for_dispatch`]; a missing/pruned saga
    /// or an unassigned attempt degrades to `"unknown"` — attribution is
    /// breadcrumb material, never a gate. `pub(crate)`: the delivery path in
    /// lib.rs computes it once per delivery for the PR-body breadcrumb AND
    /// the delivered-runs ring.
    pub(crate) async fn executing_node(&self, ctx: &dyn Ctx, run_id: &str) -> String {
        let saga_id = saga_id_for_dispatch(&self.id, &crate::dispatch_id_for(run_id));
        let Ok(reply) = ctx
            .query(&self.saga, &saga_encode_query(&SagaQuery::Get { saga_id }))
            .await
        else {
            return "unknown".into();
        };
        match saga_decode_reply(&reply) {
            Ok(SagaReply::Saga(Some(view))) => view
                .assignee
                .map(|key| crate::hex(&key))
                .unwrap_or_else(|| "unknown".into()),
            _ => "unknown".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(kind: &str, text: &str, lang: Option<&str>) -> ReplyBlock {
        ReplyBlock {
            kind: kind.into(),
            text: text.into(),
            lang: lang.map(str::to_string),
        }
    }

    #[test]
    fn message_facet_renders_paragraphs_and_fenced_code() {
        let blocks = vec![
            block("paragraph", "Fix it", None),
            block("code", "let x = 1;", Some("rust")),
            block("code", "plain", None),
        ];
        assert_eq!(
            message_facet_text(&blocks),
            "Fix it\n\n```rust\nlet x = 1;\n```\n\n```\nplain\n```"
        );
        assert_eq!(message_facet_text(&[]), "");
    }

    #[test]
    fn response_title_fallback_is_clamped_to_100_chars_on_char_boundaries() {
        // plain: the first line wins.
        assert_eq!(
            derive_pr_title(None, "Fix the gate\n\nmore detail", "r"),
            "Fix the gate"
        );
        // 2-byte chars: exactly 100 CHARS survive (200 bytes — inside the
        // forge byte cap); a byte slice at 100 would split a char.
        let title = derive_pr_title(None, &"é".repeat(120), "r");
        assert_eq!(title, "é".repeat(100));
        // 4-byte chars: the forge 256-BYTE cap clamps earlier — still on a
        // char boundary (64 whole ducks == 256 bytes).
        let title = derive_pr_title(None, &"🦆".repeat(120), "r");
        assert_eq!(title, "🦆".repeat(64));
        assert!(title.len() <= FORGE_TITLE_BYTE_CAP);
    }

    #[test]
    fn bound_item_title_beats_a_pages_style_one_line_receipt() {
        let issue = "Use Forge issue titles for auto-published Agent PRs";
        let receipt = "Implemented and recorded verification on the referenced Pages block.";
        assert_eq!(derive_pr_title(Some(issue), receipt, "run-1"), issue);
    }

    #[test]
    fn bound_item_title_beats_a_long_response() {
        let issue = "Use Forge issue titles for auto-published Agent PRs";
        let response = "A long model response must stay in the body. ".repeat(200);
        assert_eq!(derive_pr_title(Some(issue), &response, "run-1"), issue);
    }

    #[test]
    fn bound_item_title_never_degrades_to_a_generic_message() {
        let issue = "Use Forge issue titles for auto-published Agent PRs";
        assert_eq!(derive_pr_title(Some(issue), "Apply agent changes", "run-1"), issue);
    }

    #[test]
    fn pr_title_falls_back_only_without_bound_item_metadata() {
        assert_eq!(
            derive_pr_title(None, "Useful response\nmore", "run-1"),
            "Useful response"
        );
        assert_eq!(derive_pr_title(None, "", "run-1"), "agent run run-1");
        assert_eq!(derive_pr_title(None, "  \n\t\n", "run-1"), "agent run run-1");
    }

    #[test]
    fn pr_body_is_the_message_plus_the_receipt_breadcrumb() {
        let receipt = crate::facets::WorkspaceReceipt {
            branch: Some("agent/x".into()),
            output_commit: Some("abc123".into()),
            ..Default::default()
        };
        assert_eq!(
            derive_pr_body("hello\nworld", "r1", &receipt, "ab12"),
            "hello\nworld\n\n---\nrun: r1\noutput: agent/x@abc123\nnode: ab12"
        );
        // a blank message: the breadcrumb block IS the body.
        assert_eq!(
            derive_pr_body("", "r1", &receipt, "unknown"),
            "---\nrun: r1\noutput: agent/x@abc123\nnode: unknown"
        );
        // honest output lines when nothing was pushed.
        let no_changes = crate::facets::WorkspaceReceipt {
            no_changes: true,
            ..Default::default()
        };
        assert_eq!(
            derive_pr_body("m", "r1", &no_changes, "unknown"),
            "m\n\n---\nrun: r1\noutput: none (no changes this run)\nnode: unknown"
        );
        let commit_failed = crate::facets::WorkspaceReceipt {
            branch: Some("agent/x".into()),
            commit_error: Some("cas".into()),
            ..Default::default()
        };
        assert_eq!(
            derive_pr_body("m", "r1", &commit_failed, "unknown"),
            "m\n\n---\nrun: r1\noutput: none (workspace commit failed)\nnode: unknown"
        );
    }

    #[test]
    fn an_oversized_body_truncates_the_message_and_keeps_the_breadcrumb() {
        // unreachable under the 32 KiB reply-blocks cap; the deterministic
        // guard keeps a regression from handing forge a rejectable op.
        let receipt = crate::facets::WorkspaceReceipt::default();
        let body = derive_pr_body(&"x".repeat(FORGE_BODY_BYTE_CAP), "r1", &receipt, "unknown");
        assert!(body.len() <= FORGE_BODY_BYTE_CAP, "body stays inside forge's cap");
        assert!(body.ends_with("---\nrun: r1\noutput: none\nnode: unknown"));
    }

    #[test]
    fn forge_caps_mirror_the_real_forge_limits() {
        // the derivation clamps against MIRRORED caps (forge is dev-only);
        // pin them so a forge cap change cannot silently reopen the abort.
        assert_eq!(FORGE_TITLE_BYTE_CAP, forge::MAX_TITLE_BYTES);
        assert_eq!(FORGE_BODY_BYTE_CAP, forge::MAX_BODY_BYTES);
    }

    /// THE REAL PATH. `emit_sink` hands the host a `Msg` targeting forge, and
    /// the host stamps every module-emitted follow-up
    /// `Origin::Module(<emitter>)` (`host/src/lib.rs`, the one construction
    /// site). So the bytes this sink emits arrive at forge under
    /// `Origin::Module("runs")` — never under a user origin. A forge that
    /// refuses that origin errors the emitted op and ABORTS the whole delivery
    /// block, which is exactly what #860 did.
    ///
    /// The dogfood e2e (`bin/node/tests/dogfood_loop_e2e.rs`) asserts this PR
    /// too, but skips wherever `/dev/kvm` or the guest artifacts are absent —
    /// which is why the break shipped. This runs anywhere `cargo test -p runs`
    /// runs.
    #[test]
    fn the_emitted_open_pr_is_accepted_by_a_real_forge_under_a_module_origin() {
        use sdk::Module as _;

        let mut base = std::env::temp_dir();
        base.push(format!("ducktape-runs-forge-sink-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let mut forge = forge::Forge::init("forge", base.clone()).unwrap();

        let exec = |forge: &mut forge::Forge, t: u64, origin: sdk::Origin, payload: Vec<u8>| {
            let mut ctx = sdk_testkit::TestCtx::with_env(sdk::Env {
                height: 0,
                consensus_time: t,
                origin,
                me: "forge".into(),
            });
            let msg = Msg {
                target: "forge".into(),
                payload,
            };
            let out = futures::executor::block_on(forge.execute(&mut ctx, &msg));
            futures::executor::block_on(forge.commit_block()).unwrap();
            out
        };

        // birth the repo and both branches: a member pushes (a ref-moving op
        // still requires an external principal — #860's owner gate, untouched).
        let member = sdk::Origin::External(vec![1u8; 32]);
        for (t, branch) in [(1u64, "main"), (2, "agent/run-1")] {
            let push = forge::encode_msg(&forge::ForgeMsg::PushRefs {
                repo: "app".into(),
                updates: vec![forge::RefUpdate {
                    ref_name: branch.into(),
                    prev_oid: None,
                    new_oid: Some(vec![0xab; 20]),
                }],
                pack_digest: Some(vec![7u8; 32]),
                cert: None,
            });
            exec(&mut forge, t, member.clone(), push).expect("push");
        }

        // the sink's own bytes, at the origin the host stamps on them.
        exec(
            &mut forge,
            3,
            sdk::Origin::Module("runs".into()),
            forge_open_pr_bytes("app", "T", "B", "agent/run-1", "main"),
        )
        .expect("a module-emitted OpenPr must not error — it aborts the block");

        let reply = futures::executor::block_on(forge.query(&forge::encode_query(
            &forge::ForgeQuery::GetItem {
                repo: "app".into(),
                number: 1,
            },
        )))
        .unwrap();
        let forge::ForgeReply::Item(Some(item)) = forge::decode_reply(&reply).unwrap() else {
            panic!("the PR must exist")
        };
        assert_eq!(
            item.summary.author,
            chat::AuthorRef::Module("runs".into()),
            "the PR is authored by the emitting MODULE, not a forged user"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn forge_sink_mirror_matches_forge_decode_msg() {
        // pin the local ForgeSinkMsg mirror against the real forge decoder so the
        // wire cannot silently drift (the reason forge is a dev-dependency).
        let bytes = forge_open_pr_bytes("app", "T", "B", "agent/x", "main");
        assert_eq!(
            forge::decode_msg(&bytes).unwrap(),
            forge::ForgeMsg::OpenPr {
                repo: "app".into(),
                title: "T".into(),
                body: "B".into(),
                source_branch: "agent/x".into(),
                target_branch: "main".into(),
            }
        );
    }
}
