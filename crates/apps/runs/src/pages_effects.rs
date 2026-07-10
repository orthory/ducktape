//! the pages effects lane (M2): `pages.comment` / `pages.set_checked` applied
//! at the run boundary from the winning attempt (X2), alongside the chat/task
//! follow-ups — but with PER-ACTION degrade: a pages action that fails its
//! grant, cap, target resolution, payload validation, or any freshness probe
//! is dropped with a breadcrumb while the run still DELIVERS (reply, other
//! effects, finalize). that is deliberately narrower than the task lane's
//! all-or-nothing validation — a page annotation is garnish, never worth
//! failing a delivery over — and mirrors the PR sink's degrade discipline.
//!
//! every emitted op must be valid BY CONSTRUCTION (the no-fail rule: a
//! follow-up pages would reject aborts the whole delivery block, forever).
//! pages can reject an `AddComment`/`SetChecked` on: a missing/non-todo
//! block, an oversized text, a squatted thread or comment id (both are
//! client-mintable by anyone), a target-mismatched thread, and a full
//! target (thread cap). each reject path is probed against committed pages
//! state here first — the same discipline as chat's `probe_reply_postable`
//! and the forge sink's branch probes.
//!
//! attribution: `AddComment` carries `as_agent` (pages refines the
//! `Module("runs")` origin into `AuthorRef::Agent`, exactly like chat);
//! `SetChecked` stores no author — a `Block` has no author field — so it is
//! origin-gated only.

use std::collections::BTreeMap;

use agent::CapRequest;

use super::response::allows;
use super::{AgentAction, AgentRecord, Ctx, Msg, PendingState, RunsModule};
use pages::{
    MAX_COMMENT_TEXT_BYTES, MAX_THREADS_PER_TARGET, PageMsg, PageQuery, PageReply,
    encode_msg as pages_encode_msg, encode_query as pages_encode_query,
};

/// whether an action belongs to this lane (and is therefore skipped by the
/// strict task-action validator and the task emitter).
pub(super) fn is_pages_action(action: &AgentAction) -> bool {
    matches!(
        action,
        AgentAction::AddPageComment { .. } | AgentAction::SetPageChecked { .. }
    )
}

/// deterministic ids for an agent comment: derived from the run id and the
/// action's index in the validated response, so every replaying node mints
/// the identical thread/comment (never randomness — X2 replay-identity).
fn page_thread_id(run_id: &str, index: usize) -> String {
    format!("agent/{run_id}/thread/{index}")
}
fn page_comment_id(run_id: &str, index: usize) -> String {
    format!("agent/{run_id}/comment/{index}")
}

impl RunsModule {
    /// apply the pages actions of a validated response. each action either
    /// emits one pages follow-up or degrades to a breadcrumb — this lane
    /// never errors and never fails the run.
    pub(super) async fn emit_pages_effects(
        &self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        actions: &[AgentAction],
    ) {
        if !actions.iter().any(is_pages_action) {
            return;
        }
        let skip = |what: &str| format!("run {run_id} pages action skipped: {what}");
        let Some(pages) = self.pages.clone() else {
            self.note(ctx, skip("no pages module wired"));
            return;
        };
        let agent = match self.agent_record(&*ctx, &entry.agent_id).await {
            Ok(Some(a)) => a,
            _ => {
                self.note(ctx, skip("agent not registered"));
                return;
            }
        };
        // threads THIS run has already staged, per target: pages reads its
        // pending overlay first, so the committed-only thread-cap probe is
        // blind to threads staged earlier in this same delivery block. a run
        // emitting several comments to a near-full target would pass every
        // committed probe, then abort the block on the sibling pages rejects
        // (TooManyThreads) — the permanent-abort R4 hole. account for them.
        let mut staged_threads: BTreeMap<String, usize> = BTreeMap::new();
        for (index, action) in actions.iter().enumerate() {
            if !is_pages_action(action) {
                continue;
            }
            let already_staged = match action {
                AgentAction::AddPageComment { target, .. } => {
                    staged_threads.get(target).copied().unwrap_or(0)
                }
                _ => 0,
            };
            match self
                .pages_action_msg(&*ctx, &pages, &agent, run_id, index, action, already_staged)
                .await
            {
                Ok(msg) => {
                    // a landed comment opens one new thread on its target;
                    // the next sibling to that target must count it.
                    if let AgentAction::AddPageComment { target, .. } = action {
                        *staged_threads.entry(target.clone()).or_default() += 1;
                    }
                    ctx.emit_msg(msg)
                }
                Err(why) => self.note(
                    ctx,
                    format!("run {run_id} pages action {index} skipped: {why}"),
                ),
            }
        }
    }

    /// one pages action as an emit-ready follow-up, or the reason it degrades.
    /// gate order: grant → target resolution → cap → payload → freshness
    /// probes. `already_staged` is how many threads this run has staged to
    /// this comment's target already (the same-block thread-cap accounting).
    #[allow(clippy::too_many_arguments, reason = "run_id + index derive the deterministic ids; already_staged is the same-block cap counter")]
    async fn pages_action_msg(
        &self,
        ctx: &dyn Ctx,
        pages: &str,
        agent: &AgentRecord,
        run_id: &str,
        index: usize,
        action: &AgentAction,
        already_staged: usize,
    ) -> Result<Msg, String> {
        let name = action.vocabulary_name();
        if !allows(agent, name) {
            return Err(format!("agent {} is not allowed to {name}", agent.agent_id));
        }
        match action {
            AgentAction::AddPageComment { target, body } => {
                if body.is_empty() {
                    return Err("comment body is empty".into());
                }
                if body.len() > MAX_COMMENT_TEXT_BYTES {
                    return Err(format!(
                        "comment body is {} bytes; the cap is {MAX_COMMENT_TEXT_BYTES}",
                        body.len()
                    ));
                }
                // target → owning page (the cap is PAGE-scoped). a page root
                // is itself a block that names itself as `page`, so GetBlock
                // resolves both anchor shapes; None == the target exists
                // nowhere — unresolvable, degrade.
                let block = self.page_block(ctx, pages, target).await?;
                self.check_pages_write(agent, &block.page)?;
                self.probe_comment_lands(ctx, pages, run_id, index, target, already_staged)
                    .await?;
                Ok(Msg {
                    target: pages.to_string(),
                    payload: pages_encode_msg(&PageMsg::AddComment {
                        thread_id: page_thread_id(run_id, index),
                        comment_id: page_comment_id(run_id, index),
                        target: target.clone(),
                        text: body.clone(),
                        // pages refines Module("runs") + as_agent into
                        // AuthorRef::Agent — the same wire chat replies use.
                        as_agent: Some(agent.agent_id.clone()),
                    }),
                })
            }
            AgentAction::SetPageChecked { block, checked } => {
                let resolved = self.page_block(ctx, pages, block).await?;
                // pages rejects SetChecked on any non-todo kind; probed here
                // so the emitted op cannot abort the delivery block.
                if resolved.kind != pages::BlockKind::Todo {
                    return Err(format!("block {block} is not a todo"));
                }
                self.check_pages_write(agent, &resolved.page)?;
                Ok(Msg {
                    target: pages.to_string(),
                    payload: pages_encode_msg(&PageMsg::SetChecked {
                        block_id: block.clone(),
                        checked: *checked,
                    }),
                })
            }
            _ => unreachable!("only pages actions reach this lane"),
        }
    }

    /// the D3 cap gate: pages_write is page-id scoped with `"*"` allowed.
    fn check_pages_write(&self, agent: &AgentRecord, page: &str) -> Result<(), String> {
        if agent.permits(&CapRequest::PagesWrite(page)) {
            Ok(())
        } else {
            Err(format!(
                "agent {} lacks pages_write for {page}",
                agent.agent_id
            ))
        }
    }

    /// resolve a target/block id against committed pages state. `Err` == the
    /// id resolves to nothing (or the lookup failed) — degrade material.
    async fn page_block(
        &self,
        ctx: &dyn Ctx,
        pages: &str,
        block_id: &str,
    ) -> Result<pages::Block, String> {
        let reply = ctx
            .query(
                pages,
                &pages_encode_query(&PageQuery::GetBlock {
                    block_id: block_id.to_string(),
                }),
            )
            .await
            .map_err(|e| format!("pages block lookup failed: {e}"))?;
        match pages::decode_reply(&reply) {
            Ok(PageReply::Block(Some(block))) => Ok(block),
            Ok(PageReply::Block(None)) => Err(format!("target does not exist: {block_id}")),
            _ => Err("unexpected pages reply for a block lookup".into()),
        }
    }

    /// prove the AddComment we are about to emit would land RIGHT NOW: the
    /// deterministic thread and comment ids are client-mintable (anyone could
    /// squat them), and the target's thread list is capped — any of those
    /// would make pages reject the follow-up and abort the delivery block.
    async fn probe_comment_lands(
        &self,
        ctx: &dyn Ctx,
        pages: &str,
        run_id: &str,
        index: usize,
        target: &str,
        already_staged: usize,
    ) -> Result<(), String> {
        let thread_id = page_thread_id(run_id, index);
        let reply = ctx
            .query(
                pages,
                &pages_encode_query(&PageQuery::CommentThread {
                    thread_id: thread_id.clone(),
                }),
            )
            .await
            .map_err(|e| format!("pages thread lookup failed: {e}"))?;
        match pages::decode_reply(&reply) {
            Ok(PageReply::CommentThread(None)) => {}
            Ok(PageReply::CommentThread(Some(_))) => {
                return Err(format!("thread id already taken: {thread_id}"));
            }
            _ => return Err("unexpected pages reply for a thread lookup".into()),
        }
        let comment_id = page_comment_id(run_id, index);
        let reply = ctx
            .query(
                pages,
                &pages_encode_query(&PageQuery::GetComment {
                    comment_id: comment_id.clone(),
                }),
            )
            .await
            .map_err(|e| format!("pages comment lookup failed: {e}"))?;
        match pages::decode_reply(&reply) {
            Ok(PageReply::Comment(None)) => {}
            Ok(PageReply::Comment(Some(_))) => {
                return Err(format!("comment id already taken: {comment_id}"));
            }
            _ => return Err("unexpected pages reply for a comment lookup".into()),
        }
        // a fresh thread still needs a slot in the target's thread list —
        // counting both committed threads and the ones this run already
        // staged to this target (else the sibling AddComment aborts).
        let reply = ctx
            .query(
                pages,
                &pages_encode_query(&PageQuery::ThreadsForTargets {
                    targets: vec![target.to_string()],
                }),
            )
            .await
            .map_err(|e| format!("pages target lookup failed: {e}"))?;
        match pages::decode_reply(&reply) {
            Ok(PageReply::CommentThreads(groups)) => {
                let taken = groups.first().map(|g| g.threads.len()).unwrap_or(0) + already_staged;
                if taken >= MAX_THREADS_PER_TARGET {
                    return Err(format!("target {target} already holds {taken} threads"));
                }
                Ok(())
            }
            _ => Err("unexpected pages reply for a target lookup".into()),
        }
    }
}
