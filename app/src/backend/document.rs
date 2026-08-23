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

use crate::pages::sync::{
    BlockOp, DocumentPlan, document_body, document_plan, document_title, stored_lines,
};

/// The page's child pages, in document order. Subpages are navigation, not
/// prose: they have no markdown spelling, so the document editor never holds
/// them and the screen lists them underneath it instead.
pub fn subpage_blocks(blocks: &[PageBlock]) -> Vec<PageBlock> {
    blocks
        .iter()
        .filter(|block| !crate::pages::sync::is_prose(block))
        .cloned()
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

/// The buffer a page opens on: its title as line 0, its blocks under it.
/// The extern boundary hands lists by value, hence the owned parameter.
pub fn page_document_text(title: String, blocks: Vec<PageBlock>) -> String {
    crate::pages::sync::page_document_text(&title, &blocks)
}

/// A live resync replaces the buffer only when it is CLEAN and the node's own
/// text actually differs. A dirty buffer is the user still typing — dropping
/// their caret (or their words) to install a remote edit is the worst thing
/// this surface can do, so a remote change simply waits for the next save.
pub fn refreshed_page_editor(
    document: iced::widget::text_editor::Content,
    title: String,
    blocks: Vec<PageBlock>,
    saved: String,
) -> iced::widget::text_editor::Content {
    match refreshed_page_text(&document.text(), &title, &blocks, &saved) {
        Some(canonical) => iced::widget::text_editor::Content::with_text(&canonical),
        None => document,
    }
}

/// The saved-baseline mirror of [`refreshed_page_editor`] — the SAME decision
/// on the SAME inputs, so the buffer and its dirty baseline move together.
///
/// Takes the buffer's TEXT (`editor_text(page_editor)` at the call site), not
/// the editor: a by-value `editor` at the extern boundary is a `Content::clone`,
/// and iced's clone is `with_text(&self.text())` — a whole second cosmic-text
/// buffer shaped under a WRITE lock on the process-global font system, per
/// live delta.
pub fn refreshed_page_saved(
    text: String,
    title: String,
    blocks: Vec<PageBlock>,
    saved: String,
) -> String {
    refreshed_page_text(&text, &title, &blocks, &saved).unwrap_or(saved)
}

fn refreshed_page_text(
    text: &str,
    title: &str,
    blocks: &[PageBlock],
    saved: &str,
) -> Option<String> {
    let dirty = text != saved;
    if dirty {
        return None;
    }
    let canonical = crate::pages::sync::page_document_text(title, blocks);
    (canonical != saved).then_some(canonical)
}

/// The buffer a context change installs: the incoming page's canonical text
/// when the page MOVED or the buffer is clean; the user's own buffer when they
/// are mid-typing in the page that merely reloaded. One decision, computed
/// once into a `let` and applied to buffer and baseline together.
pub fn install_decision(
    text: String,
    current_page: String,
    next_page: String,
    saved: String,
    canonical: String,
) -> bool {
    if current_page != next_page {
        return true;
    }
    let clean = text == saved;
    // A clean, IDENTICAL buffer is left alone: a rebuilt `Content` throws the
    // caret to the origin, and there is nothing to install.
    clean && canonical != saved
}

pub fn installed_page_editor(
    document: iced::widget::text_editor::Content,
    install: bool,
    canonical: String,
) -> iced::widget::text_editor::Content {
    match install {
        true => {
            // The buffer is another page's now: undoing across the swap would
            // restore the PREVIOUS page's text here, and a menu opened on it
            // would hang over unrelated lines.
            crate::pages::history::reset();
            crate::pages::menu::close();
            iced::widget::text_editor::Content::with_text(&canonical)
        }
        false => document,
    }
}

/// A refusal rolls the buffer back ONLY when nothing was typed since the tick
/// submitted — otherwise the rollback would eat the user's newest words along
/// with the refused edit. A kept buffer stays dirty against the canonical
/// baseline, so the refusal re-plans (and re-explains) until resolved.
pub fn rolled_back_editor(
    document: iced::widget::text_editor::Content,
    untouched: bool,
    canonical: String,
) -> iced::widget::text_editor::Content {
    match untouched {
        true => iced::widget::text_editor::Content::with_text(&canonical),
        false => document,
    }
}

/// The dirty baseline after a save settles.
///
/// A WRITE moves the baseline to the node's canonical text: anything typed
/// during the round trip — or a depth change that converges one `MoveBlock`
/// per tick — stays dirty and the next tick carries it. A NO-OP save adopts
/// the submitted text instead: the buffer said the same thing in different
/// spelling (`* item` for `- item`), and a canonical baseline there would
/// leave the tick firing forever over a difference no op can close.
pub fn saved_baseline(written: bool, canonical: String, submitted: String) -> String {
    match written {
        true => canonical,
        false => submitted,
    }
}

/// The baseline, corrected to the title that was actually SUBMITTED.
///
/// A save adopts the node's canonical text, and that text carries a title
/// somebody else may have changed while this reader was typing — a title this
/// buffer has never displayed, because the dirty guard refuses to rebuild a
/// buffer mid-sentence. Line 0 IS the title, so taking the canonical line 0
/// makes the baseline claim a sync that did not happen, and the next tick
/// reads that manufactured difference as THIS reader retitling the page,
/// writing the old name back over the rename.
///
/// `submitted` is `page_inflight_text` — the text the tick actually reconciled
/// against the node — and NOT the live buffer. The distinction is the whole
/// correctness of this function, and the handler already draws it one line
/// below (`untouched`): a reader keeps typing during the round trip. Feeding
/// the live buffer here adopts characters she has not saved into the baseline,
/// which makes the document read CLEAN, retires the tick that would have
/// written her rename, and leaves the next live fold free to rebuild the
/// buffer and erase what she typed. That is a worse bug than the one this
/// function exists to fix, in the same family.
///
/// VERBATIM rather than trimmed, because the dirty test compares these texts
/// byte for byte and a normalized line 0 costs a needless extra round trip.
/// The identical-title case returns the canonical text untouched, so the
/// ordinary path is not reshaped at all.
///
/// The buffer is deliberately NOT corrected here. Rebuilding it would throw
/// the reader's caret to the origin mid-sentence; the title on screen catches
/// up the moment the buffer goes clean and any live event folds it.
pub fn baseline_at_submitted_title(canonical: String, submitted: String) -> String {
    let submitted_line = submitted
        .split_once('\n')
        .map_or(submitted.as_str(), |(head, _)| head);
    let title_agrees = document_title(&canonical) == document_title(submitted_line);
    if title_agrees {
        return canonical;
    }
    match canonical.split_once('\n') {
        Some((_, body)) => format!("{submitted_line}\n{body}"),
        None => submitted_line.to_string(),
    }
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
    pub written: bool,
    pub refusal: String,
    pub data: PagesData,
    pub document: String,
}

/// Whether this save owes the node a title write.
///
/// A TITLE IS ONLY "MOVED" WHEN THIS READER MOVED IT. Disagreeing with the node
/// was the whole test, and it is not enough: the node disagrees just as loudly
/// when SOMEONE ELSE renamed the page and this buffer has not caught up. Line 0
/// IS the title, so a reader who never touched it still carries the old one —
/// and the write then reverted the other person's rename on chain, silently, on
/// the next keystroke. It needs no dirty buffer and no race: any reader whose
/// line 0 is behind the chain does it.
///
/// `saved` is the baseline the buffer was last synced to, so the difference
/// between its line 0 and the buffer's is exactly "what this reader typed".
/// BOTH conditions are required and neither alone is sufficient: authorship
/// stops a stale reader from writing, and disagreement stops an agreeing title
/// from costing a block. (Authorship alone would also submit a rename on the
/// first save of a buffer whose baseline is still empty.)
pub(crate) fn title_write_owed(title: &str, saved: &str, node_title: &str) -> bool {
    let node_disagrees = title != node_title;
    let reader_retitled_it = title != document_title(saved);
    node_disagrees && reader_retitled_it
}

/// Reconcile the edited buffer against the page as the node currently holds it.
pub async fn save_page_document(
    rpc: String,
    password: String,
    page_id: String,
    text: String,
    saved: String,
) -> Result<DocumentSaveResult, AppError> {
    let client = rpc_client(&rpc).map_err(app_error)?;
    if page_id.is_empty() {
        return Err(app_error("choose a page first".into()));
    }

    // THE PLAN IS DIFFED AGAINST THIS, so it must not predate this reader's
    // own last tick: `load_pages_data` waits for the fold to carry every write
    // this client has made (`backend/rpc.rs`). A tree missing the line the
    // previous tick inserted is not merely stale — `document_plan` pairs the
    // disturbed middle POSITIONALLY, so the line comes back as a second
    // `InsertBlock` and the document ends up holding it twice.
    let current = load_pages_data(&client, Some(&page_id))
        .await
        .map_err(app_error)?;
    // A SAVE MUST PLAN AGAINST THE PAGE ITS CALLER NAMED.
    //
    // `load_pages_data` drops a requested id the index does not hold and falls
    // back to `pages.first()` (backend/load.rs). That is RIGHT for a live
    // refresh — a page can be archived out from under a resync — and
    // catastrophic here: the plan below pairs THIS buffer against those blocks
    // POSITIONALLY (`document_plan`), so every op it emits carries the OTHER
    // page's block ids. A short buffer leaves no survivors and the plan becomes
    // a `RemoveBlock` for every line of a document the reader never opened.
    //
    // Nothing downstream catches it. The title write is the only accidental
    // guard — it fails `BlockNotFound` and aborts — and it does not run at all
    // when the two titles match, which two untitled pages always do.
    //
    // The reader is not stuck: the failure surfaces as the save error, and the
    // resync that follows a delete moves `active_page` off the dead page, which
    // parks the autosave tick on `active_page != buffer_page` (handlers/pages.ice).
    if current.active_page != page_id {
        return Err(app_error("page was not found".into()));
    }
    let canonical = |data: &PagesData| {
        crate::pages::sync::page_document_text(&data.active_page_title, &data.blocks)
    };
    let stored = stored_lines(&current.blocks);
    let wanted = document_body(&text);
    let DocumentPlan { ops, refusal } = document_plan(&stored, &wanted);

    if !refusal.is_empty() {
        return Ok(DocumentSaveResult {
            written: false,
            refusal,
            document: canonical(&current),
            data: current,
        });
    }

    // The title is line 0 of the same buffer but a page property on the wire,
    // so it gets its own write — before the body, so a rename lands even if a
    // block op is refused after it. A DIRECT write, not `debounced_page_text`:
    // the save tick is already the debounce, and the debouncer's supersede
    // path returns Ok(false) — a silently dropped rename to a caller that
    // does not read the bool.
    let title = document_title(&text);
    let title_moved = title_write_owed(&title, &saved, &current.active_page_title);
    if title_moved {
        let bounded = bounded_exact_text(title, "page title", 512).map_err(app_error)?;
        write(
            &client,
            &password,
            PageMsg::UpdateText {
                block_id: page_id.clone(),
                text: bounded,
                marks: None,
            },
        )
        .await
        .map_err(app_error)?;
    }
    if ops.is_empty() && !title_moved {
        return Ok(DocumentSaveResult {
            written: false,
            refusal: String::new(),
            document: canonical(&current),
            data: current,
        });
    }
    if ops.is_empty() {
        let data = load_selected_page_data(&client, &page_id)
            .await
            .map_err(committed_error)?;
        return Ok(DocumentSaveResult {
            written: true,
            refusal: String::new(),
            document: canonical(&data),
            data,
        });
    }

    let mut anchor = String::new();

    for (index, op) in ops.iter().enumerate() {
        // Only the first body op before any title write may report an
        // uncommitted failure. Once one write has landed the page has already
        // moved, so the caller must resync either way.
        let committed_so_far = title_moved || index > 0;
        let message = apply_op(&client, &password, &page_id, &mut anchor, op).await;
        if let Err(cause) = message {
            let mark = match committed_so_far {
                true => committed_error(cause),
                false => app_error(cause),
            };
            return Err(mark);
        }
    }

    let data = load_selected_page_data(&client, &page_id)
        .await
        .map_err(committed_error)?;
    Ok(DocumentSaveResult {
        written: true,
        refusal: String::new(),
        document: canonical(&data),
        data,
    })
}

/// One op, awaited. `anchor` carries the id an insert chain hangs off: the
/// block just inserted becomes the anchor for the next one.
///
/// Three of these arms read the live tree before deciding what to write, and
/// the op before them has already moved it. They read it through
/// `load_page_blocks`, which waits for the fold to carry every block this
/// client knows about — including the write this loop made one iteration ago
/// (`backend/rpc.rs`, `SEEN_BLOCKS`) — so the ordering this loop is awaited
/// for survives the trip through the index.
async fn apply_op(
    client: &RpcClient,
    password: &str,
    page_id: &str,
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

/// One signed op onto the pages module. The receipt height is not carried by
/// hand: `signed_write` records it against the module, and every read that
/// must not answer behind it — this save's own next op, the reload that ends
/// it, the NEXT tick's plan — waits on that record instead (`backend/rpc.rs`,
/// `SEEN_BLOCKS`). Threading it by hand reached the first two and never the
/// third, which is the one that duplicates a line.
async fn write(client: &RpcClient, password: &str, msg: PageMsg) -> Result<(), String> {
    signed_write(
        client,
        "pages",
        pages::encode_msg(&msg),
        password.to_string(),
    )
    .await
    .map(|_height| ())
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
///
/// An insert that anchors on nothing lands under the page's own record — and
/// `after` is never that record's id, which is what the fallback rests on.
/// `blocks` DOES carry the page head (element 0 of `load_page_blocks`; it is
/// `page_blocks` that skips it), but the plan only ever anchors on lines
/// `page_blocks` produced or on ids this save just inserted. Were the head
/// matched, the parent adopted would be the page's OWN parent and a first-line
/// insert would land in the enclosing page; failing to find it is exactly what
/// makes an unanchored insert land under this page.
async fn insert_block(
    client: &RpcClient,
    password: &str,
    page_id: &str,
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
        .unwrap_or_else(|| page_id.to_string());
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
