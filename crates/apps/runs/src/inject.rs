//! deterministic forge item-context injection (M1): the instructions section
//! a forge-channel run's envelope carries in its `context` field, rendered
//! from COMMITTED tracker state at compose height (I1) and byte-capped.
//!
//! the wording is part of the committed prompt input (the envelope JSON is
//! the provider prompt), so the render is a pure function of the item record
//! + coordinates: same committed state, same bytes, on every validator.

use crate::forge_source::{ForgeItem, ForgeItemKind};

/// the item-context byte cap (spec verbatim: 16 KiB, truncate-with-marker —
/// never fail on size). separate from — and earlier than — the whole-payload
/// dispatch cap, which stays the final guard.
pub(crate) const MAX_CONTEXT_BYTES: usize = 16 * 1024;

/// the deterministic truncation marker; the capped render always ends with it.
const TRUNCATION_MARKER: &str = "\n[item context truncated at 16 KiB]";

/// render the deterministic instructions section for a forge item run: item
/// kind/number/state, title, body, repo coordinates, the work branch, and —
/// for a PR — its source/target branches.
pub(crate) fn render_item_context(repo: &str, item: &ForgeItem, work_branch: &str) -> String {
    let mut out = format!(
        "Forge item context — you are working this item as a session.\n\
         repo: {repo}\n\
         item: {} #{} ({})\n\
         title: {}\n\
         work branch: {work_branch}\n",
        item.kind.as_str(),
        item.number,
        item.state.as_str(),
        item.title,
    );
    if item.kind == ForgeItemKind::Pr {
        if let Some(src) = &item.source_branch {
            out.push_str(&format!("pr source branch: {src}\n"));
        }
        if let Some(tgt) = &item.target_branch {
            out.push_str(&format!("pr target branch: {tgt}\n"));
        }
    }
    out.push_str("\nItem body:\n");
    out.push_str(&item.body);
    truncate_with_marker(out)
}

/// enforce the byte cap: within it, the render is untouched; beyond it, cut
/// at the largest char boundary that leaves room for the marker, then append
/// the marker — deterministic, and NEVER a failure.
fn truncate_with_marker(s: String) -> String {
    if s.len() <= MAX_CONTEXT_BYTES {
        return s;
    }
    let budget = MAX_CONTEXT_BYTES - TRUNCATION_MARKER.len();
    let mut cut = budget;
    while !s.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut out = s[..cut].to_string();
    out.push_str(TRUNCATION_MARKER);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge_source::{ForgeItem, ForgeItemKind, ForgeItemState};

    fn issue(body: &str) -> ForgeItem {
        ForgeItem {
            number: 7,
            kind: ForgeItemKind::Issue,
            title: "Fix the flaky gate".into(),
            state: ForgeItemState::Open,
            body: body.into(),
            source_branch: None,
            target_branch: None,
        }
    }

    fn pr() -> ForgeItem {
        ForgeItem {
            number: 8,
            kind: ForgeItemKind::Pr,
            title: "Wire the thing".into(),
            state: ForgeItemState::Open,
            body: "please review".into(),
            source_branch: Some("feature/x".into()),
            target_branch: Some("dev".into()),
        }
    }

    #[test]
    fn an_issue_context_carries_coordinates_title_branch_and_body() {
        let ctx = render_item_context("app", &issue("repro:\n- run it twice"), "agent/item-7");
        assert!(ctx.contains("repo: app"), "{ctx}");
        assert!(ctx.contains("issue #7"), "{ctx}");
        assert!(ctx.contains("(open)"), "{ctx}");
        assert!(ctx.contains("title: Fix the flaky gate"), "{ctx}");
        assert!(ctx.contains("work branch: agent/item-7"), "{ctx}");
        assert!(ctx.contains("repro:\n- run it twice"), "{ctx}");
        assert!(
            !ctx.contains("source branch") && !ctx.contains("target branch"),
            "an issue renders no PR branch lines: {ctx}"
        );
    }

    #[test]
    fn a_pr_context_carries_its_source_and_target_branches() {
        let ctx = render_item_context("app", &pr(), "feature/x");
        assert!(ctx.contains("pr #8"), "{ctx}");
        assert!(ctx.contains("pr source branch: feature/x"), "{ctx}");
        assert!(ctx.contains("pr target branch: dev"), "{ctx}");
        assert!(ctx.contains("work branch: feature/x"), "{ctx}");
    }

    #[test]
    fn rendering_is_byte_deterministic() {
        let a = render_item_context("app", &issue("body"), "agent/item-7");
        let b = render_item_context("app", &issue("body"), "agent/item-7");
        assert_eq!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn an_oversized_body_truncates_to_the_cap_with_a_marker() {
        let ctx = render_item_context("app", &issue(&"x".repeat(64 * 1024)), "agent/item-7");
        assert_eq!(
            ctx.len(),
            MAX_CONTEXT_BYTES,
            "the capped render fills the budget exactly for ascii input"
        );
        assert!(
            ctx.ends_with(TRUNCATION_MARKER),
            "a truncated context states so: …{}",
            &ctx[ctx.len() - 60..]
        );
        assert!(ctx.starts_with("Forge item context"), "{ctx}");
    }

    #[test]
    fn a_context_within_the_cap_is_untouched() {
        let ctx = render_item_context("app", &issue("short body"), "agent/item-7");
        assert!(ctx.len() <= MAX_CONTEXT_BYTES);
        assert!(!ctx.contains(TRUNCATION_MARKER));
    }

    #[test]
    fn truncation_respects_utf8_boundaries() {
        // a body of multibyte chars — the byte budget will not land on a char
        // boundary; the cut must back up, never split a char.
        let ctx = render_item_context("app", &issue(&"é".repeat(32 * 1024)), "agent/item-7");
        assert!(ctx.len() <= MAX_CONTEXT_BYTES);
        assert!(ctx.ends_with(TRUNCATION_MARKER));
        // re-validating utf-8 is implicit — `ctx` is a String — but pin the
        // budget is nearly filled (backed up at most one char, not one chunk).
        assert!(ctx.len() > MAX_CONTEXT_BYTES - 4);
    }

    #[test]
    fn truncation_is_deterministic() {
        let a = render_item_context("app", &issue(&"y".repeat(64 * 1024)), "agent/item-7");
        let b = render_item_context("app", &issue(&"y".repeat(64 * 1024)), "agent/item-7");
        assert_eq!(a.as_bytes(), b.as_bytes());
    }
}
