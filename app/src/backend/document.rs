//! The page document's write path: one buffer in, the module's own ops out.
//!
//! THE ORDERING RULE. `/v1/submit` is one independent request per op and the
//! node mempools each as it arrives, so two writes fired in the same tick can
//! land either way round — and an insert chained on the block before it is
//! exactly the pattern that breaks when they do. Every op here is `await`ed
//! before the next is built, inside ONE async fn, which is what makes the chain
//! safe. Do not turn this loop into a `join_all`.
//!
//! The plan itself (what changed, and what is refused) is
//! [`crate::pages::sync`] — pure, and unit-tested without a node.

use super::*;

use crate::pages::sync::{BlockOp, DocumentPlan, document_plan, parse_document, stored_lines};

/// The page's child pages, in document order. Subpages are navigation, not
/// prose: they have no markdown spelling, so the document editor never holds
/// them and the screen lists them underneath it instead.
pub fn subpage_blocks(blocks: Vec<PageBlock>) -> Vec<PageBlock> {
    blocks
        .into_iter()
        .filter(|block| !crate::pages::sync::is_prose(block))
        .collect()
}

/// `12` — a count beside a label. Zero reads as nothing at all, because a
/// badge that says "0" is louder than the absence it reports.
pub fn count_label(count: i64) -> String {
    match count > 0 {
        true => count.to_string(),
        false => String::new(),
    }
}

/// The document text a page's blocks render as (see `crate::pages::sync`).
/// The extern boundary hands lists by value, hence the owned parameter.
pub fn page_markdown(blocks: Vec<PageBlock>) -> String {
    crate::pages::sync::page_markdown(&blocks)
}

/// A live resync replaces the buffer only when it is CLEAN and the node's own
/// text actually differs. A dirty buffer is the user still typing — dropping
/// their caret (or their words) to install a remote edit is the worst thing
/// this surface can do, so a remote change simply waits for the next save.
pub fn refreshed_page_editor(
    document: iced::widget::text_editor::Content,
    blocks: Vec<PageBlock>,
    saved: String,
) -> iced::widget::text_editor::Content {
    match refreshed_page_text(&document, &blocks, &saved) {
        Some(canonical) => iced::widget::text_editor::Content::with_text(&canonical),
        None => document,
    }
}

/// The saved-baseline mirror of [`refreshed_page_editor`] — the SAME decision
/// on the SAME inputs, so the buffer and its dirty baseline move together.
pub fn refreshed_page_saved(
    document: iced::widget::text_editor::Content,
    blocks: Vec<PageBlock>,
    saved: String,
) -> String {
    refreshed_page_text(&document, &blocks, &saved).unwrap_or(saved)
}

fn refreshed_page_text(
    document: &iced::widget::text_editor::Content,
    blocks: &[PageBlock],
    saved: &str,
) -> Option<String> {
    let dirty = crate::pages::page_text(document.clone()) != saved;
    if dirty {
        return None;
    }
    let canonical = crate::pages::sync::page_markdown(blocks);
    (canonical != saved).then_some(canonical)
}

/// The document after a save attempt.
///
/// `refusal` is not an error: the node is fine, the WRITE was not attempted
/// because carrying it out would have destroyed records (see
/// [`DocumentPlan::refusal`]). The caller shows it and takes `document` — the
/// canonical text — as the buffer's new contents, which is what rolls the
/// illegal edit back.
#[derive(Clone, Debug)]
pub struct DocumentSaveResult {
    pub generation: i64,
    pub written: bool,
    pub refusal: String,
    pub data: PagesData,
    pub document: String,
}

/// Reconcile the edited buffer against the page as the node currently holds it.
pub async fn save_page_document(
    rpc: String,
    password: String,
    page_id: String,
    text: String,
    generation: i64,
) -> Result<DocumentSaveResult, HydrationError> {
    let failed = |message: String| HydrationError {
        generation,
        message,
    };
    let client = rpc_client(&rpc).map_err(failed)?;
    if page_id.is_empty() {
        return Err(failed("choose a page first".into()));
    }

    let current = load_pages_data(&client, Some(&page_id))
        .await
        .map_err(failed)?;
    let stored = stored_lines(&current.blocks);
    let wanted = parse_document(&text);
    let DocumentPlan { ops, refusal } = document_plan(&stored, &wanted);

    if !refusal.is_empty() {
        return Ok(DocumentSaveResult {
            generation,
            written: false,
            refusal,
            document: crate::pages::sync::page_markdown(&current.blocks),
            data: current,
        });
    }
    if ops.is_empty() {
        return Ok(DocumentSaveResult {
            generation,
            written: false,
            refusal: String::new(),
            document: crate::pages::sync::page_markdown(&current.blocks),
            data: current,
        });
    }

    // The head of the page, for an insert that anchors on nothing.
    let page_head = current
        .blocks
        .first()
        .filter(|block| block.kind == "Page")
        .map(|block| block.id.clone())
        .unwrap_or_else(|| page_id.clone());
    let mut anchor = String::new();

    for (index, op) in ops.iter().enumerate() {
        // Only the FIRST op may report an uncommitted failure; once one write
        // has landed the page has already moved, so the caller must resync
        // either way.
        let committed_so_far = index > 0;
        let message = apply_op(&client, &password, &page_id, &page_head, &mut anchor, op).await;
        if let Err(cause) = message {
            let mark = match committed_so_far {
                true => committed_error(cause),
                false => app_error(cause),
            };
            return Err(failed(mark.message));
        }
    }

    let data = load_selected_page_data(&client, &page_id, "")
        .await
        .map_err(failed)?;
    Ok(DocumentSaveResult {
        generation,
        written: true,
        refusal: String::new(),
        document: crate::pages::sync::page_markdown(&data.blocks),
        data,
    })
}

/// One op, awaited. `anchor` carries the id an insert chain hangs off: the
/// block just inserted becomes the anchor for the next one.
async fn apply_op(
    client: &RpcClient,
    password: &str,
    page_id: &str,
    page_head: &str,
    anchor: &mut String,
    op: &BlockOp,
) -> Result<(), String> {
    match op {
        BlockOp::SetText { id, text } => {
            // The module bounds text per KIND; the plan never changes both in
            // the same op, so the stored kind is the one to bound against.
            let stored = block_kind(client, page_id, id).await?;
            let text = bounded_updated_block_text(stored, text.clone())?;
            write(
                client,
                password,
                PageMsg::UpdateText {
                    block_id: id.clone(),
                    text,
                    marks: None,
                },
            )
            .await
        }
        BlockOp::SetKind { id, kind } => {
            let kind = parse_block_kind(kind)?;
            write(
                client,
                password,
                PageMsg::SetKind {
                    block_id: id.clone(),
                    kind,
                },
            )
            .await
        }
        BlockOp::SetChecked { id, checked } => {
            write(
                client,
                password,
                PageMsg::SetChecked {
                    block_id: id.clone(),
                    checked: *checked,
                },
            )
            .await
        }
        BlockOp::Insert { after, kind, text } => {
            let landed = insert_block(
                client,
                password,
                page_id,
                page_head,
                match after.is_empty() {
                    true => anchor.as_str(),
                    false => after.as_str(),
                },
                kind,
                text,
            )
            .await?;
            *anchor = landed;
            Ok(())
        }
        BlockOp::Nest { id, direction } => {
            // `block_move` resolves the direction against the LIVE tree and
            // carries the divider-parent guard; the plan never names a parent.
            let blocks = load_page_blocks(client, page_id).await?;
            let (parent, after) = block_move(&blocks, id, direction)?;
            write(
                client,
                password,
                PageMsg::MoveBlock {
                    block_id: id.clone(),
                    parent,
                    after,
                },
            )
            .await
        }
        BlockOp::Remove { id } => {
            write(
                client,
                password,
                PageMsg::RemoveBlock {
                    block_id: id.clone(),
                },
            )
            .await
        }
    }
}

/// One signed op onto the pages module.
async fn write(client: &RpcClient, password: &str, msg: PageMsg) -> Result<(), String> {
    signed_write(
        client,
        "pages",
        pages::encode_msg(&msg),
        password.to_string(),
    )
    .await
}

/// The kind a block currently wears, for the module's per-kind text bound.
async fn block_kind(
    client: &RpcClient,
    page_id: &str,
    block_id: &str,
) -> Result<BlockKind, String> {
    let blocks = load_page_blocks(client, page_id).await?;
    blocks
        .iter()
        .find(|block| block.id == block_id)
        .map(|block| block.kind)
        .ok_or_else(|| "block was not found".to_string())
}

/// Insert one block after `after`, adopting that block's parent — the depth of
/// a new line is the depth of the line above it, never inferred from the text.
async fn insert_block(
    client: &RpcClient,
    password: &str,
    page_id: &str,
    page_head: &str,
    after: &str,
    kind: &str,
    text: &str,
) -> Result<String, String> {
    let kind = parse_block_kind(kind)?;
    let text = bounded_new_block_text(kind, text.to_string())?;
    let blocks = load_page_blocks(client, page_id).await?;
    let anchor = blocks.iter().find(|block| block.id == after);
    let parent = anchor
        .and_then(|block| block.parent.clone())
        .unwrap_or_else(|| page_head.to_string());
    let id = fresh_id("block");
    signed_write(
        client,
        "pages",
        pages::encode_msg(&PageMsg::InsertBlock {
            parent,
            after: anchor.map(|block| block.id.clone()),
            block: NewBlock {
                id: id.clone(),
                kind,
                text,
                marks: Vec::new(),
            },
        }),
        password.to_string(),
    )
    .await?;
    Ok(id)
}
