//! Prepare page annotations. Invalid or stale page effects produce diagnostics
//! while other valid result facets can proceed. The account's program later
//! executes each prepared message, and pages stamps the actual account actor
//! under its normal write gate.

use super::action_requests::Prepared;
use super::catalog::{ContentPart, Operation, PageAnchor};
use super::response::ReplyPosts;
use super::{Ctx, Lane, Msg, PendingState, ReplyDestination, RunsModule};
use pages::{
    BlockKind, NewBlock, PageMsg, PageQuery, PageReply, encode_msg as pages_encode_msg,
    encode_query as pages_encode_query,
};

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
/// the page a `pages.post` operation mints, one per run and lane slot: a
/// replay mints the identical id, and pages treats a re-create of an existing
/// id as a no-op, so a retried slot never doubles the page.
pub(super) fn page_post_id(run_id: &str, slot: &str) -> String {
    format!("agent/{}/page/{slot}", crate::dispatch_id_for(run_id))
}

/// the body a page post carries: one block per content part, text as a
/// paragraph and code as a code block, each id minted under the page in
/// document order. blank parts are dropped, as a reply drops blank blocks.
fn page_body(page_id: &str, content: &[ContentPart]) -> Vec<NewBlock> {
    content
        .iter()
        .filter_map(|part| match part {
            ContentPart::Text { text } => {
                let text = text.trim();
                (!text.is_empty()).then(|| (BlockKind::Paragraph, text.to_string()))
            }
            ContentPart::Code { text, .. } => {
                (!text.trim().is_empty()).then(|| (BlockKind::Code, text.clone()))
            }
        })
        .enumerate()
        .map(|(index, (kind, text))| NewBlock {
            id: format!("{page_id}/b{index}"),
            kind,
            text,
            marks: Vec::new(),
        })
        .collect()
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
        for (index, operation) in operations.iter().enumerate() {
            if !operation.is_pages() {
                continue;
            }
            match self
                .pages_operation_msg(&*ctx, entry, run_id, &lane.slot(index), operation, posts)
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
    /// not be emitted. order: target resolution → payload → freshness probes.
    ///
    /// THE ONE pages gate. the settle path degrades an `Err` here to a
    /// breadcrumb (a page annotation is garnish, never worth failing a delivery
    /// over); the session lane returns it to the submitter as an error. same
    /// verdict, two failure policies — never two verdicts.
    pub(super) async fn pages_operation_msg(
        &self,
        ctx: &dyn Ctx,
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
            Operation::PagesPost { title, content } => {
                let name = operation.name();
                let pages = self
                    .pages
                    .as_deref()
                    .ok_or("no pages module is configured")?;
                let page_id = page_post_id(run_id, slot);
                let title = title.trim();
                if title.is_empty() {
                    return Err(format!("{name} requires a non-empty title"));
                }
                if title.len() > pages::MAX_PAGE_TITLE_LEN {
                    return Err(format!(
                        "page title is {} bytes; pages' cap is {}",
                        title.len(),
                        pages::MAX_PAGE_TITLE_LEN
                    ));
                }
                let blocks = page_body(&page_id, content);
                // pages refuses a block record over its size cap and a page
                // past its page cap; both probed here so the emitted op
                // cannot abort the delivery block.
                if let Some(block) = blocks.iter().find(|b| b.text.len() > pages::MAX_BLOCK_LEN) {
                    return Err(format!(
                        "page block {} is {} bytes; pages' cap is {}",
                        block.id,
                        block.text.len(),
                        pages::MAX_BLOCK_LEN
                    ));
                }
                self.reserve_page_slot(ctx, pages, posts).await?;
                Ok(Prepared::new(
                    Msg {
                        target: pages.to_string(),
                        payload: pages_encode_msg(&PageMsg::CreatePage {
                            page_id: page_id.clone(),
                            title: title.to_string(),
                            blocks,
                        }),
                    },
                    name,
                    serde_json::json!({"page_id": page_id, "title": title}),
                ))
            }
            _ => unreachable!("only pages operations reach this lane"),
        }
    }

    /// count one more page against pages' cap: the committed page count plus
    /// every create this same block has already staged.
    async fn reserve_page_slot(
        &self,
        ctx: &dyn Ctx,
        pages: &str,
        posts: &mut ReplyPosts,
    ) -> Result<(), String> {
        let reply = ctx
            .query(pages, &pages_encode_query(&PageQuery::PageCount))
            .await
            .map_err(|e| format!("pages page count failed: {e}"))?;
        let Ok(PageReply::PageCount(count)) = pages::decode_reply(&reply) else {
            return Err("unexpected pages reply for a page count".into());
        };
        let full = count as usize + posts.pages_created >= pages::MAX_PAGES;
        if full {
            return Err("pages is full".into());
        }
        posts.pages_created += 1;
        Ok(())
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
