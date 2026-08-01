//! The document text and the block records are the same page in two shapes.
//! This module is the translation, both ways, plus the plan that turns an
//! edited buffer back into the module's own ops.
//!
//! WHY THERE IS NO TREE DIFF HERE. `RemoveBlock` deletes the whole SUBTREE
//! (pages `block_ops::delete_subtree`), and the submit path gives no ordering
//! guarantee across separate requests. A general "reconcile any two trees"
//! engine would have to reparent, and getting that wrong destroys committed
//! records on an append-only chain. So nesting is NEVER inferred from the text:
//! an inserted line adopts the depth of the line above it, depth changes stay
//! explicit `MoveBlock` ops from Tab/Shift+Tab, and a removal that would take a
//! block still holding children is REFUSED rather than guessed at.
//!
//! The pairing is a prefix/suffix trim, not an LCS: equal lines at the head and
//! tail keep their block ids (so a comment anchored to a paragraph survives an
//! edit three lines above it), and only the disturbed middle is paired by
//! position. A wholesale reshuffle costs some ids; it never costs text.

use crate::backend::PageBlock;

/// Two spaces per depth, matching `load::block_prefix`.
const INDENT: &str = "  ";
const FENCE: &str = "```";

/// One line of the document, resolved to the block vocabulary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub kind: String,
    pub text: String,
    pub checked: bool,
    pub depth: usize,
}

/// A line that is already a record, carrying the id the ops address it by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredLine {
    pub id: String,
    pub has_children: bool,
    pub line: Line,
}

/// One write the document owes the node. Applied strictly in order.
///
/// The three in-place variants are separate ON PURPOSE: each is its own signed
/// transaction, so a fat "update everything" op would bill three writes for
/// fixing one typo. The plan emits only the fields that actually moved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockOp {
    SetText {
        id: String,
        text: String,
    },
    SetKind {
        id: String,
        kind: String,
    },
    SetChecked {
        id: String,
        checked: bool,
    },
    /// A new line. `after` is the block it follows, empty for the page head.
    Insert {
        after: String,
        kind: String,
        text: String,
    },
    /// A line whose indentation moved. ONE step per plan: `block_move`
    /// resolves a direction against the live tree, so a two-step drag
    /// converges over consecutive save ticks rather than guessing a parent.
    Nest {
        id: String,
        direction: String,
    },
    /// A line that is gone. Only ever emitted for a childless block.
    Remove {
        id: String,
    },
}

/// What the buffer wants written, or why it cannot be.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DocumentPlan {
    pub ops: Vec<BlockOp>,
    /// Non-empty means: write NOTHING and resync. The document asked for
    /// something the module cannot do without losing records.
    pub refusal: String,
}

/// Subpages are navigation, not prose — they have no markdown spelling, and a
/// text diff has no business deciding they were deleted. They are rendered
/// beside the document and skipped by every function here.
pub fn is_prose(block: &PageBlock) -> bool {
    block.kind != "Page"
}

/// The document text for a page's blocks.
pub fn page_markdown(blocks: &[PageBlock]) -> String {
    let lines: Vec<String> = blocks
        .iter()
        .filter(|block| is_prose(block))
        .map(|block| {
            render_line(&Line {
                kind: block.kind.clone(),
                text: block.text.clone(),
                checked: block.checked,
                depth: block.prefix.len() / INDENT.len(),
            })
        })
        .collect();
    lines.join("\n")
}

/// The stored shape of a page's prose, in document order.
pub fn stored_lines(blocks: &[PageBlock]) -> Vec<StoredLine> {
    blocks
        .iter()
        .filter(|block| is_prose(block))
        .map(|block| StoredLine {
            id: block.id.clone(),
            has_children: block.child_count > 0,
            line: Line {
                kind: block.kind.clone(),
                text: block.text.clone(),
                checked: block.checked,
                depth: block.prefix.len() / INDENT.len(),
            },
        })
        .collect()
}

/// One block as its markdown line (or lines — a Code block is a fence).
fn render_line(line: &Line) -> String {
    let indent = INDENT.repeat(line.depth);
    let marker = match line.kind.as_str() {
        "Heading 1" => "# ",
        "Heading 2" => "## ",
        "Heading 3" => "### ",
        "Bullet" => "- ",
        "Number" => "1. ",
        "Todo" => match line.checked {
            true => "- [x] ",
            false => "- [ ] ",
        },
        // `+` is a legal CommonMark bullet, reserved here for Toggle so the
        // kind survives a round trip instead of degrading into Bullet.
        "Toggle" => "+ ",
        "Quote" => "> ",
        "Callout" => "!> ",
        "Divider" => return format!("{indent}---"),
        "Code" => {
            let body: Vec<String> = line
                .text
                .lines()
                .map(|body| format!("{indent}{body}"))
                .collect();
            let body = body.join("\n");
            return match body.is_empty() {
                true => format!("{indent}{FENCE}\n{indent}{FENCE}"),
                false => format!("{indent}{FENCE}\n{body}\n{indent}{FENCE}"),
            };
        }
        _ => "",
    };
    format!("{indent}{marker}{}", line.text)
}

/// The document text, resolved back into lines. A fenced run folds into ONE
/// Code line carrying the body verbatim, which is why this cannot be a `map`.
pub fn parse_document(text: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut source = text.lines().peekable();
    while let Some(raw) = source.next() {
        let trimmed = raw.trim_start_matches([' ', '\t']);
        let depth = (raw.len() - trimmed.len()) / INDENT.len();
        if !trimmed.starts_with(FENCE) {
            lines.push(parse_line(raw, trimmed, depth));
            continue;
        }
        // A code body is VERBATIM past the fence's own indent. Trimming all
        // leading whitespace here would silently reformat every indented line
        // of every code block on the first save.
        let own_indent = INDENT.repeat(depth);
        let mut body = Vec::new();
        for inside in source.by_ref() {
            if inside.trim_start_matches([' ', '\t']).starts_with(FENCE) {
                break;
            }
            let stripped = inside.strip_prefix(&own_indent).unwrap_or(inside);
            body.push(stripped.to_string());
        }
        lines.push(Line {
            kind: "Code".into(),
            text: body.join("\n"),
            checked: false,
            depth,
        });
    }
    lines
}

fn parse_line(raw: &str, trimmed: &str, depth: usize) -> Line {
    let plain = |kind: &str, text: &str, checked: bool| Line {
        kind: kind.into(),
        text: text.into(),
        checked,
        depth,
    };
    if trimmed.trim_end() == "---" {
        return plain("Divider", "", false);
    }
    // Ordered longest-first: `### ` must not be read as `# ` plus prose.
    let markers = [
        ("### ", "Heading 3"),
        ("## ", "Heading 2"),
        ("# ", "Heading 1"),
        ("- [x] ", "Todo"),
        ("- [X] ", "Todo"),
        ("- [ ] ", "Todo"),
        ("!> ", "Callout"),
        ("> ", "Quote"),
        ("+ ", "Toggle"),
        ("- ", "Bullet"),
        ("* ", "Bullet"),
    ];
    for (marker, kind) in markers {
        let Some(rest) = trimmed.strip_prefix(marker) else {
            continue;
        };
        let checked = marker.eq_ignore_ascii_case("- [x] ");
        return plain(kind, rest, checked);
    }
    if let Some(rest) = ordered_content(trimmed) {
        return plain("Number", rest, false);
    }
    plain("Text", raw.trim_start_matches([' ', '\t']), false)
}

/// `12. text` / `12) text` — the content past an ordered marker. The stored
/// number is positional, so the digits themselves are not kept.
fn ordered_content(trimmed: &str) -> Option<&str> {
    let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let rest = trimmed.get(digits..)?;
    let rest = rest.strip_prefix('.').or_else(|| rest.strip_prefix(')'))?;
    rest.strip_prefix(' ')
}

/// The writes that carry `stored` to `wanted`.
///
/// Head and tail runs that are already equal are skipped, so their ids — and
/// anything anchored to them — never move. The disturbed middle is paired by
/// position: the overlap updates in place, a stored surplus is removed, a
/// wanted surplus is inserted after the last block that survives ahead of it.
pub fn document_plan(stored: &[StoredLine], wanted: &[Line]) -> DocumentPlan {
    let common_head = stored
        .iter()
        .zip(wanted)
        .take_while(|(have, want)| have.line == **want)
        .count();
    let tail_room = stored.len().min(wanted.len()) - common_head;
    let common_tail = stored
        .iter()
        .rev()
        .zip(wanted.iter().rev())
        .take(tail_room)
        .take_while(|(have, want)| have.line == **want)
        .count();

    let stored_middle = &stored[common_head..stored.len() - common_tail];
    let wanted_middle = &wanted[common_head..wanted.len() - common_tail];

    // A block that still holds children cannot be removed — `RemoveBlock` takes
    // the subtree with it. Refusing is the only honest answer; the caller
    // resyncs and says so.
    let doomed_parent = stored_middle
        .iter()
        .skip(wanted_middle.len())
        .find(|stored| stored.has_children);
    if let Some(parent) = doomed_parent {
        return DocumentPlan {
            ops: Vec::new(),
            refusal: format!(
                "\"{}\" still has sub-items — delete those first",
                summarize(&parent.line.text)
            ),
        };
    }

    let mut ops = Vec::new();
    for (have, want) in stored_middle.iter().zip(wanted_middle) {
        // Depth becomes a `MoveBlock` direction, never a guessed parent.
        if have.line.depth != want.depth {
            ops.push(BlockOp::Nest {
                id: have.id.clone(),
                direction: match want.depth > have.line.depth {
                    true => "indent".into(),
                    false => "outdent".into(),
                },
            });
        }
        if have.line.text != want.text {
            ops.push(BlockOp::SetText {
                id: have.id.clone(),
                text: want.text.clone(),
            });
        }
        if have.line.kind != want.kind {
            ops.push(BlockOp::SetKind {
                id: have.id.clone(),
                kind: want.kind.clone(),
            });
        }
        if have.line.checked != want.checked {
            ops.push(BlockOp::SetChecked {
                id: have.id.clone(),
                checked: want.checked,
            });
        }
    }
    for surplus in stored_middle.iter().skip(wanted_middle.len()) {
        ops.push(BlockOp::Remove {
            id: surplus.id.clone(),
        });
    }

    // An insert anchors on the last block that is still there ahead of it. The
    // stored middle's own survivors come first, then the head run.
    let survivors = stored_middle.len().min(wanted_middle.len());
    let mut anchor = stored_middle
        .get(survivors.wrapping_sub(1))
        .map(|stored| stored.id.clone())
        .or_else(|| {
            stored
                .get(common_head.wrapping_sub(1))
                .map(|stored| stored.id.clone())
        })
        .unwrap_or_default();
    for fresh in wanted_middle.iter().skip(stored_middle.len()) {
        ops.push(BlockOp::Insert {
            after: anchor.clone(),
            kind: fresh.kind.clone(),
            text: fresh.text.clone(),
        });
        // Each insert anchors on the one before it, and the caller applies the
        // list strictly in order, so the chain resolves as it is written.
        anchor = String::new();
    }

    DocumentPlan {
        ops,
        refusal: String::new(),
    }
}

fn summarize(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "this block".into();
    }
    match trimmed.char_indices().nth(28) {
        Some((cut, _)) => format!("{}…", &trimmed[..cut]),
        None => trimmed.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(kind: &str, text: &str) -> Line {
        Line {
            kind: kind.into(),
            text: text.into(),
            checked: false,
            depth: 0,
        }
    }

    fn stored(id: &str, kind: &str, text: &str) -> StoredLine {
        StoredLine {
            id: id.into(),
            has_children: false,
            line: line(kind, text),
        }
    }

    #[test]
    fn every_block_kind_survives_a_round_trip() {
        let kinds = [
            ("Text", "plain"),
            ("Heading 1", "one"),
            ("Heading 2", "two"),
            ("Heading 3", "three"),
            ("Bullet", "point"),
            ("Number", "first"),
            ("Toggle", "fold"),
            ("Quote", "said"),
            ("Callout", "note"),
            ("Divider", ""),
            ("Code", "let x = 1;"),
        ];
        for (kind, text) in kinds {
            let source = line(kind, text);
            let rendered = render_line(&source);
            let parsed = parse_document(&rendered);
            assert_eq!(
                parsed,
                vec![source],
                "{kind} did not round-trip: {rendered}"
            );
        }
    }

    #[test]
    fn a_ticked_todo_keeps_its_tick_through_the_round_trip() {
        let done = Line {
            checked: true,
            ..line("Todo", "shipped")
        };
        assert_eq!(render_line(&done), "- [x] shipped");
        assert_eq!(parse_document("- [x] shipped"), vec![done]);
    }

    #[test]
    fn a_multi_line_code_block_folds_back_into_one_block() {
        let code = line("Code", "fn main() {\n    go();\n}");
        let rendered = render_line(&code);
        assert_eq!(rendered, "```\nfn main() {\n    go();\n}\n```");
        // The body's OWN indentation is content, not layout — a round trip
        // that reformats it has eaten the user's code.
        assert_eq!(parse_document(&rendered), vec![code]);
    }

    #[test]
    fn a_nested_code_block_loses_its_nesting_indent_and_keeps_its_own() {
        let code = Line {
            depth: 1,
            ..line("Code", "if x:\n    go()")
        };
        let rendered = render_line(&code);
        assert_eq!(rendered, "  ```\n  if x:\n      go()\n  ```");
        assert_eq!(parse_document(&rendered), vec![code]);
    }

    #[test]
    fn markdown_inside_a_fence_is_code_not_a_heading() {
        let parsed = parse_document("```\n# not a heading\n```");
        assert_eq!(parsed, vec![line("Code", "# not a heading")]);
    }

    #[test]
    fn depth_round_trips_as_two_spaces_per_level() {
        let nested = Line {
            depth: 2,
            ..line("Bullet", "deep")
        };
        assert_eq!(render_line(&nested), "    - deep");
        assert_eq!(parse_document("    - deep"), vec![nested]);
    }

    #[test]
    fn an_untouched_document_writes_nothing() {
        let have = vec![stored("a", "Text", "one"), stored("b", "Text", "two")];
        let want = vec![line("Text", "one"), line("Text", "two")];
        assert_eq!(document_plan(&have, &want).ops, Vec::new());
    }

    #[test]
    fn editing_one_line_updates_only_that_block() {
        let have = vec![
            stored("a", "Text", "one"),
            stored("b", "Text", "two"),
            stored("c", "Text", "three"),
        ];
        let want = vec![
            line("Text", "one"),
            line("Text", "TWO"),
            line("Text", "three"),
        ];
        assert_eq!(
            document_plan(&have, &want).ops,
            vec![BlockOp::SetText {
                id: "b".into(),
                text: "TWO".into(),
            }]
        );
    }

    #[test]
    fn typing_a_hash_promotes_the_block_in_place_and_keeps_its_id() {
        let have = vec![stored("a", "Text", "Title")];
        let want = vec![line("Heading 1", "Title")];
        // Only the KIND moved, so only one write is billed.
        assert_eq!(
            document_plan(&have, &want).ops,
            vec![BlockOp::SetKind {
                id: "a".into(),
                kind: "Heading 1".into(),
            }]
        );
    }

    #[test]
    fn indenting_a_line_moves_it_one_step_rather_than_reparenting_it() {
        let have = vec![stored("a", "Bullet", "one"), stored("b", "Bullet", "two")];
        let want = vec![
            line("Bullet", "one"),
            Line {
                depth: 1,
                ..line("Bullet", "two")
            },
        ];
        assert_eq!(
            document_plan(&have, &want).ops,
            vec![BlockOp::Nest {
                id: "b".into(),
                direction: "indent".into(),
            }]
        );
    }

    #[test]
    fn outdenting_is_the_same_move_in_the_other_direction() {
        let have = vec![StoredLine {
            line: Line {
                depth: 2,
                ..line("Bullet", "deep")
            },
            ..stored("a", "Bullet", "deep")
        }];
        let want = vec![Line {
            depth: 1,
            ..line("Bullet", "deep")
        }];
        assert_eq!(
            document_plan(&have, &want).ops,
            vec![BlockOp::Nest {
                id: "a".into(),
                direction: "outdent".into(),
            }]
        );
    }

    #[test]
    fn ticking_a_todo_writes_only_the_tick() {
        let have = vec![stored("a", "Todo", "ship it")];
        let want = vec![Line {
            checked: true,
            ..line("Todo", "ship it")
        }];
        assert_eq!(
            document_plan(&have, &want).ops,
            vec![BlockOp::SetChecked {
                id: "a".into(),
                checked: true,
            }]
        );
    }

    #[test]
    fn a_new_middle_line_anchors_on_the_block_above_it() {
        let have = vec![stored("a", "Text", "one"), stored("b", "Text", "three")];
        let want = vec![
            line("Text", "one"),
            line("Text", "two"),
            line("Text", "three"),
        ];
        assert_eq!(
            document_plan(&have, &want).ops,
            vec![BlockOp::Insert {
                after: "a".into(),
                kind: "Text".into(),
                text: "two".into(),
            }]
        );
    }

    #[test]
    fn a_deleted_line_removes_exactly_its_own_block() {
        let have = vec![
            stored("a", "Text", "one"),
            stored("b", "Text", "two"),
            stored("c", "Text", "three"),
        ];
        let want = vec![line("Text", "one"), line("Text", "three")];
        assert_eq!(
            document_plan(&have, &want).ops,
            vec![BlockOp::Remove { id: "b".into() }]
        );
    }

    #[test]
    fn removing_a_parent_is_refused_rather_than_taking_its_subtree() {
        let parent = StoredLine {
            has_children: true,
            ..stored("b", "Text", "parent of things")
        };
        let have = vec![stored("a", "Text", "one"), parent];
        let want = vec![line("Text", "one")];
        let plan = document_plan(&have, &want);
        assert!(plan.ops.is_empty(), "a refused plan writes nothing");
        assert!(plan.refusal.contains("still has sub-items"), "{plan:?}");
    }

    #[test]
    fn appending_to_the_end_anchors_on_the_last_stored_block() {
        let have = vec![stored("a", "Text", "one")];
        let want = vec![line("Text", "one"), line("Bullet", "next")];
        assert_eq!(
            document_plan(&have, &want).ops,
            vec![BlockOp::Insert {
                after: "a".into(),
                kind: "Bullet".into(),
                text: "next".into(),
            }]
        );
    }

    #[test]
    fn the_first_line_of_an_empty_page_anchors_on_nothing() {
        let want = vec![line("Text", "hello")];
        assert_eq!(
            document_plan(&[], &want).ops,
            vec![BlockOp::Insert {
                after: String::new(),
                kind: "Text".into(),
                text: "hello".into(),
            }]
        );
    }

    fn block(kind: &str, text: &str, depth: usize) -> PageBlock {
        PageBlock {
            key: 0,
            id: text.into(),
            parent: String::new(),
            kind: kind.into(),
            text: text.into(),
            pending: false,
            checked: false,
            prefix: INDENT.repeat(depth),
            child_count: 0,
            mark_count: 0,
            spans: Vec::new(),
        }
    }

    #[test]
    fn subpages_are_not_prose_and_never_reach_the_document() {
        assert!(!is_prose(&block("Page", "a child page", 0)));
        assert!(is_prose(&block("Text", "prose", 0)));
        let blocks = [
            block("Text", "before", 0),
            block("Page", "a child page", 0),
            block("Text", "after", 0),
        ];
        assert_eq!(page_markdown(&blocks), "before\nafter");
        assert_eq!(stored_lines(&blocks).len(), 2);
    }

    #[test]
    fn a_page_s_blocks_render_as_the_document_the_editor_opens_on() {
        let blocks = [
            block("Heading 1", "Title", 0),
            block("Text", "A paragraph.", 0),
            block("Bullet", "first", 0),
            block("Bullet", "nested", 1),
        ];
        assert_eq!(
            page_markdown(&blocks),
            "# Title\nA paragraph.\n- first\n  - nested"
        );
        // ...and the document reads back as the same lines it was built from.
        assert_eq!(
            parse_document(&page_markdown(&blocks)),
            stored_lines(&blocks)
                .into_iter()
                .map(|stored| stored.line)
                .collect::<Vec<_>>()
        );
    }
}
