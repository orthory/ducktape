//! Pages wire and platform adapter.

use super::*;

pub(super) async fn load_pages(
    backend: Option<&Backend>,
    client: Option<NodeClient>,
    active: Option<&str>,
    open_tabs: Vec<String>,
) -> Result<Option<PagesData>, String> {
    let client = client.ok_or_else(|| "enter a network to load Pages".to_string())?;
    let reply = client
        .query("pages", json!("list_pages"))
        .await
        .map_err(|error| error.to_string())?;
    let pages = variant_array(&reply, "page_list")?
        .iter()
        .filter_map(parse_page_meta)
        .collect::<Vec<_>>();
    if pages.is_empty() {
        Ok(None)
    } else {
        let document = match active.filter(|active| pages.iter().any(|page| page.id == *active)) {
            Some(active) => {
                let mut document = load_page(Some(client.clone()), active).await?;
                document.ancestry = page_ancestry(&pages, active);
                document.self_key = match backend {
                    Some(backend) => backend.identity_state().await?.pubkey,
                    None => None,
                };
                Some(document)
            }
            None => None,
        };
        Ok(Some(PagesData {
            open_tabs: open_tabs
                .into_iter()
                .filter(|tab| pages.iter().any(|page| &page.id == tab))
                .collect(),
            pages,
            document,
        }))
    }
}

async fn load_page(client: Option<NodeClient>, page: &str) -> Result<PageDocument, String> {
    let client = client.ok_or_else(|| "enter a network to load Pages".to_string())?;
    let reply = client
        .query("pages", json!({ "get_page": { "page_id": page } }))
        .await
        .map_err(|error| error.to_string())?;
    let wire = reply
        .get("page")
        .and_then(Value::as_array)
        .ok_or_else(|| "page was not found".to_string())?;
    let root = wire
        .first()
        .ok_or_else(|| "page contains no root block".to_string())?;
    let title = root
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or("Untitled")
        .to_string();
    let mut parents = std::collections::HashMap::new();
    for value in wire {
        if let (Some(id), Some(parent)) = (
            value.get("id").and_then(Value::as_str),
            value.get("parent").and_then(Value::as_str),
        ) {
            parents.insert(id, parent);
        }
    }
    let blocks = wire
        .iter()
        .skip(1)
        .filter_map(|value| parse_page_block(value, &parents))
        .collect::<Vec<_>>();
    let targets = std::iter::once(page.to_string())
        .chain(blocks.iter().take(511).map(|block| block.id.clone()))
        .collect::<Vec<_>>();
    let comments = client
        .query(
            "pages",
            json!({ "threads_for_targets": { "targets": targets } }),
        )
        .await
        .map_err(|error| error.to_string())?;
    let comment_threads = parse_page_comments(&comments)?;
    let page_comments = comment_threads
        .iter()
        .filter(|thread| thread.target == page)
        .count();
    Ok(PageDocument {
        id: page.to_string(),
        title,
        ancestry: Vec::new(),
        blocks,
        page_comments,
        comment_threads,
        presence: Vec::new(),
        self_key: None,
    })
}

pub(super) async fn load_page_with_ancestry(
    backend: Option<&Backend>,
    client: Option<NodeClient>,
    page: &str,
) -> Result<PageDocument, String> {
    let client = client.ok_or_else(|| "enter a network to load Pages".to_string())?;
    let mut document = load_page(Some(client.clone()), page).await?;
    document.self_key = match backend {
        Some(backend) => backend.identity_state().await?.pubkey,
        None => None,
    };
    let reply = client
        .query("pages", json!("list_pages"))
        .await
        .map_err(|error| error.to_string())?;
    let pages = variant_array(&reply, "page_list")?
        .iter()
        .filter_map(parse_page_meta)
        .collect::<Vec<_>>();
    document.ancestry = page_ancestry(&pages, page);
    Ok(document)
}

pub(super) fn page_ancestry(pages: &[PageMeta], page: &str) -> Vec<PageMeta> {
    let mut ancestry = Vec::new();
    let mut cursor = pages
        .iter()
        .find(|candidate| candidate.id == page)
        .and_then(|candidate| candidate.parent.as_deref());
    while let Some(parent) = cursor {
        let Some(meta) = pages.iter().find(|candidate| candidate.id == parent) else {
            break;
        };
        ancestry.push(meta.clone());
        cursor = meta.parent.as_deref();
        if ancestry.len() >= pages.len() {
            return Vec::new();
        }
    }
    ancestry.reverse();
    ancestry
}

pub(super) async fn create_page(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    page_id: String,
    parent: Option<String>,
) -> Result<(), String> {
    pages_write(
        backend,
        client,
        json!({
            "create_page": {
                "page_id": page_id,
                "title": "Untitled",
                "parent": parent
            }
        }),
    )
    .await
}

pub(super) async fn apply_slash(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    block: String,
    kind: BlockKind,
    text: String,
) -> Result<(), String> {
    pages_write(
        backend,
        client,
        json!({ "update_text": { "block_id": &block, "text": text } }),
    )
    .await?;
    pages_write(
        backend,
        client,
        json!({ "set_kind": { "block_id": block, "kind": block_kind_wire(kind) } }),
    )
    .await
}

pub(super) async fn pages_write(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    payload: Value,
) -> Result<(), String> {
    user_content_service::pages_write(backend, client, payload).await
}

pub(super) async fn split_page_block(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    left: PageBlock,
    right: PageBlock,
    thread_moves: Vec<ThreadMove>,
) -> Result<(), String> {
    // Keep the original full block authoritative until the right half and all
    // of its exact comment anchors have landed. Any failure leaves a visible
    // duplicate, never silently discarded text.
    pages_write(
        backend,
        client,
        json!({
            "insert_block": {
                "parent": right.parent,
                "after": left.id,
                "block": {
                    "id": right.id,
                    "kind": block_kind_wire(right.kind),
                    "text": right.text,
                    "marks": marks_wire(&right.marks)
                }
            }
        }),
    )
    .await?;
    move_page_threads(backend, client, thread_moves).await?;
    pages_write(
        backend,
        client,
        json!({
            "update_text": {
                "block_id": left.id,
                "text": left.text,
                "marks": marks_wire(&left.marks)
            }
        }),
    )
    .await
}

pub(super) async fn merge_page_block(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    destination: PageBlock,
    source: PageBlock,
    thread_moves: Vec<ThreadMove>,
) -> Result<(), String> {
    // The source remains the fallback copy until its comments and children
    // have moved. Removing it earlier would turn a rejected move into loss.
    pages_write(
        backend,
        client,
        json!({
            "update_text": {
                "block_id": destination.id,
                "text": destination.text,
                "marks": marks_wire(&destination.marks)
            }
        }),
    )
    .await?;
    move_page_threads(backend, client, thread_moves).await?;
    let mut after = destination.children.last().cloned();
    for child in &source.children {
        pages_write(
            backend,
            client,
            json!({
                "move_block": {
                    "block_id": child,
                    "parent": destination.id,
                    "after": after
                }
            }),
        )
        .await?;
        after = Some(child.clone());
    }
    pages_write(
        backend,
        client,
        json!({ "remove_block": { "block_id": source.id } }),
    )
    .await
}

async fn move_page_threads(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    moves: Vec<ThreadMove>,
) -> Result<(), String> {
    for movement in moves {
        pages_write(
            backend,
            client,
            json!({
                "move_comment_thread": {
                    "thread_id": movement.thread,
                    "target": movement.target,
                    "anchor": movement.anchor.map(anchor_wire)
                }
            }),
        )
        .await?;
    }
    Ok(())
}

pub(super) fn anchor_wire(anchor: RelativeAnchor) -> Value {
    json!({ "start": anchor.start, "end": anchor.end })
}

pub(super) fn marks_wire(marks: &[SpanMark]) -> Vec<Value> {
    marks
        .iter()
        .map(|mark| {
            json!({
                "start": mark.start,
                "end": mark.end,
                "kind": inline_mark_wire(mark.kind)
            })
        })
        .collect()
}

pub(super) async fn paste_page_blocks(
    backend: Option<&Backend>,
    client: Option<&NodeClient>,
    parent: String,
    mut after: Option<String>,
    blocks: Vec<(BlockKind, String, bool)>,
) -> Result<(), String> {
    if blocks.is_empty() || blocks.len() > 60 {
        return Err("page paste must contain between 1 and 60 blocks".into());
    }
    for (kind, text, checked) in blocks {
        let id = fresh_id("block");
        pages_write(
            backend,
            client,
            json!({
                "insert_block": {
                    "parent": parent,
                    "after": after,
                    "block": { "id": id, "kind": block_kind_wire(kind), "text": text }
                }
            }),
        )
        .await?;
        if checked {
            pages_write(
                backend,
                client,
                json!({ "set_checked": { "block_id": id, "checked": true } }),
            )
            .await?;
        }
        after = Some(id);
    }
    Ok(())
}

pub(super) const fn block_kind_wire(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Paragraph => "paragraph",
        BlockKind::Heading1 => "heading1",
        BlockKind::Heading2 => "heading2",
        BlockKind::Heading3 => "heading3",
        BlockKind::Bulleted => "bulleted",
        BlockKind::Numbered => "numbered",
        BlockKind::Todo => "todo",
        BlockKind::Toggle => "toggle",
        BlockKind::Quote => "quote",
        BlockKind::Code => "code",
        BlockKind::Callout => "callout",
        BlockKind::Divider => "divider",
    }
}

pub(super) const fn inline_mark_wire(kind: InlineMark) -> &'static str {
    match kind {
        InlineMark::Bold => "bold",
        InlineMark::Italic => "italic",
        InlineMark::Underline => "underline",
        InlineMark::Strikethrough => "strikethrough",
        InlineMark::Code => "code",
    }
}

fn parse_page_meta(value: &Value) -> Option<PageMeta> {
    Some(PageMeta {
        id: value.get("id")?.as_str()?.to_string(),
        title: value.get("title")?.as_str()?.to_string(),
        parent: value
            .get("parent")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn parse_page_block(
    value: &Value,
    parents: &std::collections::HashMap<&str, &str>,
) -> Option<PageBlock> {
    let id = value.get("id")?.as_str()?;
    let mut depth = 0;
    let mut cursor = id;
    while let Some(parent) = parents.get(cursor) {
        depth += 1;
        cursor = parent;
        if depth > parents.len() {
            return None;
        }
    }
    Some(PageBlock {
        id: id.to_string(),
        kind: match value.get("kind")?.as_str()? {
            "heading1" => BlockKind::Heading1,
            "heading2" => BlockKind::Heading2,
            "heading3" => BlockKind::Heading3,
            "bulleted" => BlockKind::Bulleted,
            "numbered" => BlockKind::Numbered,
            "todo" => BlockKind::Todo,
            "toggle" => BlockKind::Toggle,
            "quote" => BlockKind::Quote,
            "code" => BlockKind::Code,
            "callout" => BlockKind::Callout,
            "divider" => BlockKind::Divider,
            _ => BlockKind::Paragraph,
        },
        text: value
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        depth: depth.saturating_sub(1),
        checked: value
            .get("checked")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        parent: value.get("parent")?.as_str()?.to_string(),
        children: value
            .get("children")
            .and_then(Value::as_array)
            .map(|children| {
                children
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        marks: value
            .get("marks")
            .and_then(Value::as_array)
            .map(|marks| marks.iter().filter_map(parse_span_mark).collect())
            .unwrap_or_default(),
    })
}

fn parse_span_mark(value: &Value) -> Option<SpanMark> {
    Some(SpanMark {
        start: value.get("start")?.as_u64()?.try_into().ok()?,
        end: value.get("end")?.as_u64()?.try_into().ok()?,
        kind: match value.get("kind")?.as_str()? {
            "bold" => InlineMark::Bold,
            "italic" => InlineMark::Italic,
            "underline" => InlineMark::Underline,
            "strikethrough" => InlineMark::Strikethrough,
            "code" => InlineMark::Code,
            _ => return None,
        },
    })
}

pub(super) fn parse_page_comments(value: &Value) -> Result<Vec<PageCommentThread>, String> {
    let groups = variant_array(value, "comment_threads")?;
    if groups.len() > 512 {
        return Err("page comments exceed the desktop safety limit".into());
    }
    let mut output = Vec::new();
    for group in groups {
        let target = group
            .get("target")
            .and_then(Value::as_str)
            .ok_or_else(|| "node returned an invalid page comment target".to_string())?;
        let threads = group
            .get("threads")
            .and_then(Value::as_array)
            .ok_or_else(|| "node returned invalid page comment threads".to_string())?;
        for view in threads.iter().take(512usize.saturating_sub(output.len())) {
            let thread = view
                .get("thread")
                .ok_or_else(|| "node returned an invalid page comment thread".to_string())?;
            let comments = view
                .get("comments")
                .and_then(Value::as_array)
                .ok_or_else(|| "node returned invalid page comments".to_string())?;
            output.push(PageCommentThread {
                id: thread
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "node returned an invalid page comment id".to_string())?
                    .to_string(),
                target: target.to_string(),
                anchor: match thread.get("anchor") {
                    Some(Value::Object(anchor)) => {
                        let start = anchor
                            .get("start")
                            .and_then(Value::as_u64)
                            .and_then(|value| value.try_into().ok())
                            .ok_or_else(|| "node returned an invalid comment anchor".to_string())?;
                        let end = anchor
                            .get("end")
                            .and_then(Value::as_u64)
                            .and_then(|value| value.try_into().ok())
                            .ok_or_else(|| "node returned an invalid comment anchor".to_string())?;
                        if start >= end {
                            return Err("node returned an invalid comment anchor".into());
                        }
                        Some(RelativeAnchor { start, end })
                    }
                    Some(Value::Null) | None => None,
                    _ => return Err("node returned an invalid comment anchor".into()),
                },
                resolved: thread
                    .get("resolved")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                comments: comments
                    .iter()
                    .take(512)
                    .map(|comment| {
                        Ok(PageComment {
                            id: comment
                                .get("id")
                                .and_then(Value::as_str)
                                .ok_or_else(|| {
                                    "node returned an invalid page comment id".to_string()
                                })?
                                .to_string(),
                            author: author_name(comment.get("author").ok_or_else(|| {
                                "node returned an invalid page comment author".to_string()
                            })?),
                            author_key: comment
                                .get("author")
                                .and_then(|author| author.get("user"))
                                .and_then(Value::as_array)
                                .filter(|bytes| bytes.len() == 32)
                                .and_then(|bytes| wire_bytes_hex(bytes)),
                            text: comment
                                .get("text")
                                .and_then(Value::as_str)
                                .ok_or_else(|| {
                                    "node returned invalid page comment text".to_string()
                                })?
                                .to_string(),
                            deleted: comment
                                .get("deleted")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            edited: comment
                                .get("edited_at")
                                .is_some_and(|value| !value.is_null()),
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?,
            });
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_comment_anchor_and_marks_keep_exact_utf16_wire_offsets() {
        let comments = json!({
            "comment_threads": [{
                "target": "b1",
                "threads": [{
                    "thread": {
                        "id": "t1",
                        "anchor": { "start": 2, "end": 5 },
                        "resolved": false
                    },
                    "comments": []
                }]
            }]
        });
        let parsed = parse_page_comments(&comments).unwrap();
        assert_eq!(parsed[0].anchor, Some(RelativeAnchor { start: 2, end: 5 }));
        assert_eq!(
            marks_wire(&[SpanMark {
                start: 1,
                end: 3,
                kind: InlineMark::Bold
            }]),
            vec![json!({ "start": 1, "end": 3, "kind": "bold" })],
        );
        assert_eq!(
            anchor_wire(RelativeAnchor { start: 2, end: 5 }),
            json!({ "start": 2, "end": 5 }),
        );
    }

    #[test]
    fn every_page_block_kind_matches_the_module_wire() {
        let kinds = [
            (BlockKind::Paragraph, "paragraph"),
            (BlockKind::Heading1, "heading1"),
            (BlockKind::Heading2, "heading2"),
            (BlockKind::Heading3, "heading3"),
            (BlockKind::Bulleted, "bulleted"),
            (BlockKind::Numbered, "numbered"),
            (BlockKind::Todo, "todo"),
            (BlockKind::Toggle, "toggle"),
            (BlockKind::Quote, "quote"),
            (BlockKind::Code, "code"),
            (BlockKind::Callout, "callout"),
            (BlockKind::Divider, "divider"),
        ];
        for (kind, wire) in kinds {
            assert_eq!(block_kind_wire(kind), wire);
        }
    }

    #[test]
    fn page_ancestry_is_root_first_and_rejects_cycles() {
        let pages = vec![
            PageMeta {
                id: "root".into(),
                title: "Root".into(),
                parent: None,
            },
            PageMeta {
                id: "child".into(),
                title: "Child".into(),
                parent: Some("root".into()),
            },
            PageMeta {
                id: "leaf".into(),
                title: "Leaf".into(),
                parent: Some("child".into()),
            },
        ];
        assert_eq!(
            page_ancestry(&pages, "leaf")
                .iter()
                .map(|page| page.id.as_str())
                .collect::<Vec<_>>(),
            vec!["root", "child"]
        );
        assert!(
            page_ancestry(
                &[PageMeta {
                    id: "loop".into(),
                    title: "Loop".into(),
                    parent: Some("loop".into()),
                }],
                "loop"
            )
            .is_empty()
        );
    }
}
