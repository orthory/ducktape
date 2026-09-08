//! Prepare page annotations under the model's grant. Invalid or stale page
//! effects produce diagnostics while other valid result facets can proceed.
//! The account's program later executes each prepared message, and pages stamps
//! the actual account actor under its normal write gate.

use crate::CapRequest;

use super::action_requests::Prepared;
use super::catalog::{Operation, PageAnchor};
use super::response::{ReplyPosts, allows};
use super::{Ctx, Lane, ModelRecord, Msg, PendingState, ReplyDestination, RunsModule};
use pages::{PageMsg, PageQuery, PageReply, encode_msg as pages_encode_msg, encode_query as pages_encode_query};

/// deterministic ids for an agent comment: derived from the run id and the
/// action's [`Lane`] slot — its index in the validated response on the settle
/// path, its session action counter mid-run — so every replaying node mints the
/// identical thread/comment (never randomness — X2 replay-identity).
///
/// the run id is HASHED (hex sha256, via `dispatch_id_for`) rather than
/// inlined: a raw run id embeds the reserved `\x1f` run-key separator (a
/// control char) AND a loosely-bounded channel id, so the inline form would
/// be neither index-safe nor length-bounded — pages would reject it and the
/// emitted op would abort the delivery block. the 64-char hex hash is short,
/// escape-free, and still fully replay-deterministic.
pub(super) fn page_thread_id(run_id: &str, slot: &str) -> String {
    format!("agent/{}/thread/{slot}", crate::dispatch_id_for(run_id))
}
pub(super) fn page_comment_id(run_id: &str, slot: &str) -> String {
    format!("agent/{}/comment/{slot}", crate::dispatch_id_for(run_id))
}

/// the explicit destination a `pages.comment` operation names.
pub(super) fn page_destination(anchor: &PageAnchor) -> ReplyDestination {
    match anchor {
        PageAnchor::Target(target) => ReplyDestination::Page {
            target: target.clone(),
        },
        PageAnchor::Thread(thread_id) => ReplyDestination::PageThread {
            thread_id: thread_id.clone(),
        },
    }
}

impl RunsModule {
    /// apply the pages operations of a validated response. each operation
    /// either emits one pages follow-up or degrades to a breadcrumb — this lane
    /// never errors and never fails the run. `posts` carries the same-block
    /// thread and target reservations shared with the conversational lane.
    pub(super) async fn emit_pages_effects(
        &self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        entry: &PendingState,
        lane: Lane,
        operations: &[Operation],
        posts: &mut ReplyPosts,
    ) {
        if !operations.iter().any(Operation::is_pages) {
            return;
        }
        let skip = |what: &str| format!("run {run_id} pages action skipped: {what}");
        let agent = match self.agent_for_run(&*ctx, entry).await {
            Ok(Some(a)) => a,
            _ => {
                self.note(ctx, skip("agent not registered"));
                return;
            }
        };
        for (index, operation) in operations.iter().enumerate() {
            if !operation.is_pages() {
                continue;
            }
            match self
                .pages_operation_msg(&*ctx, &agent, entry, run_id, &lane.slot(index), operation, posts)
                .await
            {
                Ok(prepared) => self.emit_prepared(ctx, prepared),
                Err(why) => self.note(
                    ctx,
                    format!("run {run_id} pages action {index} skipped: {why}"),
                ),
            }
        }
    }

    /// one pages operation as an emit-ready follow-up, or the reason it must
    /// not be emitted. gate order: grant → target resolution → cap → payload →
    /// freshness probes.
    ///
    /// THE ONE pages gate. the settle path degrades an `Err` here to a
    /// breadcrumb (a page annotation is garnish, never worth failing a delivery
    /// over); the session lane returns it to the submitter as an error. same
    /// verdict, two failure policies — never two verdicts.
    #[allow(
        clippy::too_many_arguments,
        reason = "run_id + slot derive the deterministic ids; posts is the same-block reservation ledger"
    )]
    pub(super) async fn pages_operation_msg(
        &self,
        ctx: &dyn Ctx,
        agent: &ModelRecord,
        entry: &PendingState,
        run_id: &str,
        slot: &str,
        operation: &Operation,
        posts: &mut ReplyPosts,
    ) -> Result<Prepared, String> {
        match operation {
            Operation::PagesComment { anchor, content } => {
                // a comment is a conversational write with an explicit page
                // destination: the same resolver every reply goes through.
                self.reply_msg(
                    ctx,
                    run_id,
                    entry,
                    slot,
                    &crate::content_blocks(content),
                    Some(page_destination(anchor)),
                    posts,
                )
                .await
            }
            Operation::PagesSetChecked { block_id, checked } => {
                let name = operation.name();
                if !allows(agent, name) {
                    return Err(format!("agent {} is not allowed to {name}", agent.agent_id));
                }
                let pages = self
                    .pages
                    .as_deref()
                    .ok_or("no pages module is configured")?;
                let resolved = self.page_block(ctx, pages, block_id).await?;
                // pages rejects SetChecked on any non-todo kind; probed here
                // so the emitted op cannot abort the delivery block.
                if resolved.kind != pages::BlockKind::Todo {
                    return Err(format!("block {block_id} is not a todo"));
                }
                self.check_pages_write(agent, &resolved.page)?;
                Ok(Prepared::new(
                    Msg {
                        target: pages.to_string(),
                        payload: pages_encode_msg(&PageMsg::SetChecked {
                            block_id: block_id.clone(),
                            checked: *checked,
                        }),
                    },
                    name,
                    serde_json::json!({"block_id": block_id, "checked": checked}),
                ))
            }
            _ => unreachable!("only pages operations reach this lane"),
        }
    }

    /// the D3 cap gate: pages_write is page-id scoped with `"*"` allowed.
    pub(super) fn check_pages_write(&self, agent: &ModelRecord, page: &str) -> Result<(), String> {
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
    pub(super) async fn page_block(
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
}
