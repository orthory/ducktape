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

/// THE TITLE IS LINE 0. It is a page property on the wire, not a block, but in
/// the buffer it is simply the document's first line — which is what makes
/// Enter at the end of the title and Backspace at the start of the body work
/// without either being special-cased: they are ordinary text edits, and the
/// save path reads line 0 back out.
pub fn page_document_text(title: &str, blocks: &[PageBlock]) -> String {
    let body = page_markdown(blocks);
    match body.is_empty() {
        true => title.to_string(),
        false => format!("{title}\n{body}"),
    }
}

/// The title the buffer is carrying.
pub fn document_title(text: &str) -> String {
    text.lines().next().unwrap_or_default().trim().to_string()
}

/// Everything under the title, resolved into block lines.
pub fn document_body(text: &str) -> Vec<Line> {
    let body = text.split_once('\n').map_or("", |(_, body)| body);
    parse_document(body)
}

/// The document text for a page's blocks, title excluded.
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
                // `SetKind` off Todo leaves the stored flag behind; reading it
                // for other kinds would pair phantom ticks the plan can never
                // reconcile (`SetChecked` is Todo-only on the node).
                checked: block.kind == "Todo" && block.checked,
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
            // `split`, not `lines`: a code body's trailing newline is content,
            // and `lines()` eats it — the next save would then write the
            // stripped text back as a permanent edit.
            let body: Vec<String> = line
                .text
                .split('\n')
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

/// True while the buffer holds an ODD number of fence lines — an open ``` with
/// no close yet. Parsing such a buffer folds everything under the open fence
/// into one Code block, and the plan would then REMOVE the blocks that
/// "vanished"; the save tick waits instead. Line 0 is the title and is never
/// parsed, so it does not count.
pub fn has_unclosed_fence(text: &str) -> bool {
    let fences = text
        .lines()
        .skip(1)
        .filter(|line| line.trim_start_matches([' ', '\t']).starts_with(FENCE))
        .count();
    fences % 2 == 1
}

/// A line's leading whitespace as nesting steps: two spaces or one tab per
/// step, and a leftover odd space belongs to the TEXT — discarding it would
/// eat a byte of pasted prose on every save.
pub(crate) fn split_indent(raw: &str) -> (usize, &str) {
    let mut steps = 0;
    let mut pending = 0;
    let mut consumed = 0;
    for byte in raw.bytes() {
        match byte {
            b' ' => {
                pending += 1;
                if pending == 2 {
                    steps += 1;
                    pending = 0;
                }
            }
            b'\t' => {
                steps += 1;
                pending = 0;
            }
            _ => break,
        }
        consumed += 1;
    }
    (steps, &raw[consumed - pending..])
}

/// The document text, resolved back into lines. A fenced run folds into ONE
/// Code line carrying the body verbatim, which is why this cannot be a `map`.
///
/// `split('\n')`, never `lines()`: the final empty line of a document IS a
/// block (the empty paragraph a page can end on), and `lines()` eats it — the
/// plan would then remove that block just for having opened the page.
///
/// Depth is CLAMPED to the line above's depth + 1 (and the first line to 0):
/// that is the only shape the tree can hold, and an unclamped depth becomes a
/// `MoveBlock` the module rejects forever.
pub fn parse_document(text: &str) -> Vec<Line> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<Line> = Vec::new();
    let mut source = text.split('\n').peekable();
    while let Some(raw) = source.next() {
        let (steps, rest) = split_indent(raw);
        let ceiling = lines.last().map_or(0, |line| line.depth + 1);
        let depth = steps.min(ceiling);
        if !rest.starts_with(FENCE) {
            lines.push(parse_line(rest, depth));
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

fn parse_line(rest: &str, depth: usize) -> Line {
    let plain = |kind: &str, text: &str, checked: bool| Line {
        kind: kind.into(),
        text: text.into(),
        checked,
        depth,
    };
    let trimmed = rest;
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
    if let Some(content) = ordered_content(trimmed) {
        return plain("Number", content, false);
    }
    plain("Text", rest, false)
}

/// `12. text` / `12) text` — the content past an ordered marker. The stored
/// number is positional, so the digits themselves are not kept — which is
/// exactly why a LONG number is prose: "1997. A great year" written as a list
/// item would come back as "1. A great year", destroying the year.
// ponytail: <= 2 digits is a heuristic; carrying the start number through the
// module is the upgrade if 100+-item lists ever matter.
fn ordered_content(trimmed: &str) -> Option<&str> {
    let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > 2 {
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

    // A removal may take a parent ONLY when its whole subtree goes with it —
    // `RemoveBlock` is defined to take the subtree, so deleting the lines of a
    // nested list together is ONE remove on its root. A parent whose subtree
    // extends past the removed run would take survivors with it; that is
    // refused, and the caller resyncs and says so.
    let doomed_end = common_head + stored_middle.len();
    let survivors = stored_middle.len().min(wanted_middle.len());
    for (offset, doomed) in stored_middle.iter().enumerate().skip(survivors) {
        if !doomed.has_children {
            continue;
        }
        let index = common_head + offset;
        let subtree = stored[index + 1..]
            .iter()
            .take_while(|below| below.line.depth > doomed.line.depth)
            .count();
        let subtree_leaks = index + 1 + subtree > doomed_end;
        if subtree_leaks {
            return DocumentPlan {
                ops: Vec::new(),
                refusal: format!(
                    "\"{}\" still has sub-items — delete those first",
                    summarize(&doomed.line.text)
                ),
            };
        }
    }

    let mut ops = Vec::new();
    for (offset, (have, want)) in stored_middle.iter().zip(wanted_middle).enumerate() {
        // Depth becomes a `MoveBlock` direction, never a guessed parent — and
        // an indent is only asked for when the stored tree can PERFORM it (a
        // previous sibling to move under). An unperformable step is deferred:
        // the next tick re-plans against fresher state, and a plan that comes
        // back empty settles the baseline instead of retrying forever.
        if have.line.depth != want.depth {
            let indent = want.depth > have.line.depth;
            let previous_peer = stored[..common_head + offset]
                .iter()
                .rev()
                .map(|earlier| earlier.line.depth)
                .find(|depth| *depth <= have.line.depth);
            let performable = !indent || previous_peer == Some(have.line.depth);
            if performable {
                ops.push(BlockOp::Nest {
                    id: have.id.clone(),
                    direction: match indent {
                        true => "indent".into(),
                        false => "outdent".into(),
                    },
                });
            }
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
        // A tick is a Todo fact — on any other wanted kind there is nothing to
        // reconcile, and the module rejects the op (`NotTodo`).
        if want.kind == "Todo" && have.line.checked != want.checked {
            ops.push(BlockOp::SetChecked {
                id: have.id.clone(),
                checked: want.checked,
            });
        }
    }
    // REVERSE document order: a parent precedes its subtree in preorder, so
    // walking backwards removes leaves first and every parent is childless by
    // the time its own `Remove` lands — no op ever takes a survivor.
    for surplus in stored_middle.iter().skip(survivors).rev() {
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

    fn line_at(depth: usize, text: &str) -> Line {
        Line {
            depth,
            ..line("Text", text)
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
    fn an_empty_line_is_an_empty_text_block_and_round_trips() {
        // Enter-Enter — the most ordinary key in a document — makes a blank
        // paragraph. It must be writable, not a save error.
        assert_eq!(parse_document("one\n\ntwo").len(), 3);
        assert_eq!(parse_document("one\n\ntwo")[1], line("Text", ""));
        assert_eq!(render_line(&line("Text", "")), "");
    }

    #[test]
    fn a_code_body_keeps_its_trailing_newline_through_the_round_trip() {
        let code = line("Code", "x\n");
        let rendered = render_line(&code);
        assert_eq!(rendered, "```\nx\n\n```");
        assert_eq!(parse_document(&rendered), vec![code]);
    }

    #[test]
    fn an_open_fence_is_flagged_until_its_close_arrives() {
        assert!(has_unclosed_fence("Title\n```\nlet x = 1;"));
        assert!(!has_unclosed_fence("Title\n```\nlet x = 1;\n```"));
        // Line 0 is the title, never parsed — backticks there do not count.
        assert!(!has_unclosed_fence("``` in a title\nbody"));
        assert!(!has_unclosed_fence(""));
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
        let parent = line("Bullet", "setup");
        let code = Line {
            depth: 1,
            ..line("Code", "if x:\n    go()")
        };
        let rendered = format!("{}\n{}", render_line(&parent), render_line(&code));
        assert_eq!(rendered, "- setup\n  ```\n  if x:\n      go()\n  ```");
        assert_eq!(parse_document(&rendered), vec![parent, code]);
    }

    #[test]
    fn markdown_inside_a_fence_is_code_not_a_heading() {
        let parsed = parse_document("```\n# not a heading\n```");
        assert_eq!(parsed, vec![line("Code", "# not a heading")]);
    }

    #[test]
    fn depth_round_trips_as_two_spaces_per_level() {
        let ladder = "- a\n  - b\n    - c";
        let parsed = parse_document(ladder);
        assert_eq!(
            parsed.iter().map(|l| l.depth).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(
            parsed
                .iter()
                .map(render_line)
                .collect::<Vec<_>>()
                .join("\n"),
            ladder
        );
    }

    #[test]
    fn depth_is_clamped_to_what_the_tree_can_hold() {
        // An isolated deep line has nothing to nest under: the tree cannot
        // represent it, and an unclamped depth becomes a MoveBlock the module
        // rejects on every tick, forever.
        assert_eq!(parse_document("    - deep")[0].depth, 0);
        let jump = parse_document("- a\n        - way deep");
        assert_eq!(jump[1].depth, 1);
    }

    #[test]
    fn tabs_count_as_indent_steps_and_an_odd_space_stays_in_the_text() {
        let tabbed = parse_document("- a\n\t- b");
        assert_eq!(
            tabbed[1],
            Line {
                depth: 1,
                ..line("Bullet", "b")
            }
        );
        // Three spaces: one step, and the odd space belongs to the text.
        assert_eq!(split_indent("   x"), (1, " x"));
        assert_eq!(parse_document("- a\n   x")[1], line_at(1, " x"));
    }

    #[test]
    fn a_long_number_is_prose_because_its_digits_would_be_destroyed() {
        // The stored ordered marker is positional — "1997." would come back
        // as "1.", deleting the year. Two digits keep real lists working.
        assert_eq!(
            parse_document("1997. A great year")[0],
            line("Text", "1997. A great year")
        );
        assert_eq!(parse_document("12. twelfth")[0], line("Number", "twelfth"));
    }

    #[test]
    fn a_final_empty_line_is_a_block_and_survives_the_round_trip() {
        let parsed = parse_document("one\n");
        assert_eq!(parsed, vec![line("Text", "one"), line("Text", "")]);
        assert_eq!(parse_document(""), Vec::new());
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
    fn removing_a_parent_is_refused_when_its_subtree_survives() {
        let parent = StoredLine {
            has_children: true,
            ..stored("b", "Text", "parent of things")
        };
        let child = StoredLine {
            line: line_at(1, "kept child"),
            ..stored("c", "Text", "kept child")
        };
        let have = vec![stored("a", "Text", "one"), parent, child.clone()];
        let want = vec![line("Text", "one"), child.line.clone()];
        let plan = document_plan(&have, &want);
        assert!(plan.ops.is_empty(), "a refused plan writes nothing");
        assert!(plan.refusal.contains("still has sub-items"), "{plan:?}");
    }

    #[test]
    fn deleting_a_whole_subtree_together_is_allowed_leaves_first() {
        let parent = StoredLine {
            has_children: true,
            ..stored("b", "Text", "parent")
        };
        let child = StoredLine {
            line: line_at(1, "child"),
            ..stored("c", "Text", "child")
        };
        let have = vec![stored("a", "Text", "one"), parent, child];
        let want = vec![line("Text", "one")];
        assert_eq!(
            document_plan(&have, &want).ops,
            vec![
                BlockOp::Remove { id: "c".into() },
                BlockOp::Remove { id: "b".into() },
            ]
        );
    }

    #[test]
    fn an_unperformable_indent_is_deferred_not_submitted() {
        // The first body line has no previous sibling; the module would
        // reject the MoveBlock forever. An empty plan lets the baseline
        // settle instead.
        let have = vec![stored("a", "Bullet", "one")];
        let want = vec![Line {
            depth: 1,
            ..line("Bullet", "one")
        }];
        assert_eq!(document_plan(&have, &want).ops, Vec::new());
    }

    #[test]
    fn unticking_a_todo_by_retyping_its_kind_writes_no_phantom_tick() {
        let done = StoredLine {
            line: Line {
                checked: true,
                ..line("Todo", "was done")
            },
            ..stored("a", "Todo", "was done")
        };
        let want = vec![line("Bullet", "was done")];
        // Only the kind moves — SetChecked is Todo-only on the node, and a
        // Bullet has no tick to reconcile.
        assert_eq!(
            document_plan(&[done], &want).ops,
            vec![BlockOp::SetKind {
                id: "a".into(),
                kind: "Bullet".into(),
            }]
        );
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
        }
    }

    #[test]
    fn the_title_is_line_zero_and_the_body_starts_under_it() {
        let blocks = [block("Text", "first", 0), block("Bullet", "second", 0)];
        let text = page_document_text("My page", &blocks);
        assert_eq!(text, "My page\nfirst\n- second");
        assert_eq!(document_title(&text), "My page");
        assert_eq!(
            document_body(&text),
            vec![line("Text", "first"), line("Bullet", "second")]
        );
    }

    #[test]
    fn a_title_only_page_has_no_body_and_no_stray_blank_line() {
        let text = page_document_text("Just a title", &[]);
        assert_eq!(text, "Just a title");
        assert_eq!(document_title(&text), "Just a title");
        assert_eq!(document_body(&text), Vec::new());
    }

    #[test]
    fn markdown_on_the_title_line_stays_literal_because_a_title_has_no_kind() {
        // `# ` on line 0 is part of the title's own text, not a heading marker:
        // `document_body` never sees line 0, so nothing can parse it.
        let text = "# Not a heading\nbody";
        assert_eq!(document_title(text), "# Not a heading");
        assert_eq!(document_body(text), vec![line("Text", "body")]);
    }

    #[test]
    fn emptying_the_buffer_leaves_an_empty_title_and_no_blocks() {
        assert_eq!(document_title(""), "");
        assert_eq!(document_body(""), Vec::new());
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
