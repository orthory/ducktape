//! canonical serialization: `Document` -> deterministic bytes.
//!
//! this is the version-stable on-disk storage contract — the bytes a document
//! serializes to for git. treat it like an on-disk format, not a helper: the
//! output shape is a promise to every future reader of the repository.

use crate::Document;

/// serialize a document to its canonical byte form: walk the node linked list
/// in order and join each node's `render` with `'\n'`.
///
/// # determinism
///
/// walks [`Document::nodes_iter`] (the forward linked list), never the backing
/// `HashMap` directly — `HashMap` iteration order is not stable across runs.
/// each node renders deterministically (frontmatter sorts its misc keys), so the
/// same document always produces the same bytes. there is no trailing newline:
/// the output is a pure join, and a trailing newline would not survive reparse
/// (`BufReader::lines` drops it), which would break idempotence.
///
/// # the round-trip contract is *byte idempotence*
///
/// for any **parser-producible** document,
/// `canonical(parse(canonical(doc))) == canonical(doc)`.
///
/// it is byte idempotence, not value equality, because uids are minted fresh on
/// every parse and never appear on disk — they cannot and need not round-trip.
/// the stored bytes are the identity, so byte stability is the property that
/// matters to the storage layer.
///
/// # precondition / known limitations (v1 format has no escaping)
///
/// op-produced documents (e.g. via [`Document::apply`]'s `OnUserWrite`) can hold
/// content the parser cannot read back. `canonical` still emits it faithfully,
/// but the bytes then do **not** round-trip:
///
/// 1. a `Body` whose text has a line starting with `/` or equal to `---`:
///    reparsing stops at that line and **silently drops every node after it**
///    (worst case — no error). pinned by
///    `known_limitation_command_like_body_line_truncates_on_reparse`.
/// 2. a trailing newline on the **final** `Body` node: dropped on reparse, so
///    the bytes are not idempotent. pinned by
///    `known_limitation_trailing_newline_on_final_body_not_idempotent`.
/// 3. adjacent `Body` nodes: would merge into one on reparse. currently latent —
///    no public path constructs a `Body` node (ops can only edit existing text),
///    so it becomes reachable only once op producers can insert body nodes.
/// 4. a comment/task body line equal to its closer (`/comment.v1` / `/task.v1`):
///    closes the group early on reparse, then fails parsing the remainder (a
///    loud error, unlike case 1's silent loss).
///
/// normalizing or validating op-documents before they are checkpointed to git is
/// a downstream (B6) decision — see the git-substrate handoff. `canonical`
/// itself stays faithful: it never silently rewrites a document's content.
pub fn canonical(doc: &Document) -> Vec<u8> {
    doc.nodes_iter()
        .map(|node| node.render())
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::{Op, OpId};
    use nodes::Nodes;
    use uid::Identify;

    // a document already in canonical form: frontmatter first, no leading or
    // trailing whitespace, misc-free, structured nodes interleaved with prose.
    // because it IS canonical, `canonical(parse(it))` must reproduce it exactly.
    const CANONICAL_FIXTURE: &str = "\
---
title: test doc
author: @orthory
created_at: 1234
updated_at: 5678
---
intro prose
spanning two lines
/comment.v1{@orthory;1;2}
a comment body
/comment.v1
between nodes
/task.v1{@author;ship it;InProgress(https://example.com);10;20;@a;@b}
task body line
/task.v1
trailing prose";

    #[test]
    fn canonical_reproduces_canonical_input_exactly() {
        let doc = Document::from_reader(CANONICAL_FIXTURE.as_bytes()).expect("parse");
        assert_eq!(
            canonical(&doc),
            CANONICAL_FIXTURE.as_bytes(),
            "canonical of an already-canonical document must be the input verbatim"
        );
    }

    #[test]
    fn canonical_is_byte_idempotent_and_preserves_node_separation() {
        let doc = Document::from_reader(CANONICAL_FIXTURE.as_bytes()).expect("parse");
        let node_count = doc.nodes_iter().count();
        assert_eq!(node_count, 6, "frontmatter + 3 bodies + comment + task");

        let c1 = canonical(&doc);
        let doc2 = Document::from_reader(&c1[..]).expect("reparse");
        let c2 = canonical(&doc2);

        assert_eq!(c1, c2, "canonical must be byte-idempotent");
        assert_eq!(
            doc2.nodes_iter().count(),
            node_count,
            "node separation must survive the round-trip — not collapse or merge"
        );
    }

    // a document whose only content is a frontmatter (the minimal valid doc)
    // still round-trips.
    #[test]
    fn canonical_round_trips_minimal_frontmatter_only() {
        let input = "---\ntitle: t\nauthor: @a\ncreated_at: 0\nupdated_at: 0\n---";
        let doc = Document::from_reader(input.as_bytes()).expect("parse");
        let c1 = canonical(&doc);
        assert_eq!(c1, input.as_bytes());
        let doc2 = Document::from_reader(&c1[..]).expect("reparse");
        assert_eq!(canonical(&doc2), c1);
    }

    // misc keys survive the round-trip and are emitted deterministically (the
    // document-level confirmation of frontmatter's per-node sorting).
    #[test]
    fn canonical_is_stable_for_frontmatter_misc() {
        let input =
            "---\ntitle: t\nauthor: @a\ncreated_at: 0\nupdated_at: 0\nbeta: 2\nalpha: 1\n---";
        let doc = Document::from_reader(input.as_bytes()).expect("parse");
        let c1 = canonical(&doc);
        // alpha sorts before beta regardless of HashMap insertion order.
        let text = String::from_utf8(c1.clone()).unwrap();
        assert!(text.contains("alpha: 1\nbeta: 2"));
        let doc2 = Document::from_reader(&c1[..]).expect("reparse");
        assert_eq!(canonical(&doc2), c1, "misc ordering is idempotent");
    }

    // ---- known limitations: op-produced docs the v1 format cannot read back ----
    // these pin the "serializable != re-derivable" seam so it's discovered here,
    // in test names, rather than in production. they are NOT bugs in `canonical`
    // (it emits faithfully) — they are properties of an escaping-free format.

    fn first_body_uid(doc: &Document) -> uid::Uid {
        doc.nodes_iter()
            .find(|n| matches!(n, Nodes::Body(_)))
            .expect("fixture has a body")
            .uid()
    }

    #[test]
    fn known_limitation_command_like_body_line_truncates_on_reparse() {
        // op-produced: a user types a line starting with '/' into a body.
        let mut doc = Document::from_reader(CANONICAL_FIXTURE.as_bytes()).expect("parse");
        let before = doc.nodes_iter().count();
        let body = first_body_uid(&doc);

        doc.apply(Op::OnUserWrite {
            op_id: OpId::new(1, 1),
            node_id: body,
            pos: 0,
            text: "/danger\n".into(),
        })
        .expect("apply");

        // canonical emits the '/danger' line verbatim; on reparse the body
        // breaks at it and every following node is silently dropped.
        let bytes = canonical(&doc);
        let reparsed = Document::from_reader(&bytes[..]).expect("reparse still succeeds");
        assert!(
            reparsed.nodes_iter().count() < before,
            "a '/'-prefixed body line silently truncates the document on reparse"
        );
    }

    #[test]
    fn known_limitation_trailing_newline_on_final_body_not_idempotent() {
        let mut doc = Document::from_reader(CANONICAL_FIXTURE.as_bytes()).expect("parse");

        // the last node is the "trailing prose" body; append a newline to it.
        let last = doc.nodes_iter().last().expect("non-empty");
        assert!(matches!(last, Nodes::Body(_)), "fixture ends in a body");
        let last_uid = last.uid();
        let end = match &last {
            Nodes::Body(b) => b.text.len(),
            _ => unreachable!(),
        };

        doc.apply(Op::OnUserWrite {
            op_id: OpId::new(1, 1),
            node_id: last_uid,
            pos: end,
            text: "\n".into(),
        })
        .expect("apply");

        // canonical preserves the trailing '\n'; reparse drops it (lines() yields
        // no trailing empty), so the bytes are not idempotent.
        let c1 = canonical(&doc);
        let doc2 = Document::from_reader(&c1[..]).expect("reparse");
        let c2 = canonical(&doc2);
        assert_ne!(
            c1, c2,
            "a trailing newline on the final body does not round-trip"
        );
    }
}
