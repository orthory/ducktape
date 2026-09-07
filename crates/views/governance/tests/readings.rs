//! The readings the screen folds off the register, pinned where they moved
//! from (the desktop app's `backend/shell.rs`).

use governance_view::host::{
    ProposalRow, approve_label, proposal_kind_tone, proposals_summary, quorum_dots, tally_label,
    tally_note, tally_tone,
};

fn proposal(open: bool) -> ProposalRow {
    ProposalRow {
        open,
        ..ProposalRow::default()
    }
}

/// A subtitle that counts nothing says nothing: the plate already says it in
/// words. Something there, every subtitle speaks, zeros included.
#[test]
fn the_subtitle_is_silent_over_an_empty_register() {
    assert_eq!(proposals_summary(true, &[]), "");
    assert_eq!(proposals_summary(false, &[proposal(true)]), "");
    assert_eq!(
        proposals_summary(true, &[proposal(true)]),
        "1 open · 0 settled"
    );
    assert_eq!(
        proposals_summary(true, &[proposal(false)]),
        "0 open · 1 settled"
    );
}

#[test]
fn quorum_dots_count_the_frozen_rule_not_the_electorate() {
    // three of the four REQUIRED signatures are in, inside a six-node pool.
    let dots = quorum_dots(3, 4);
    assert_eq!(dots.len(), 4);
    assert_eq!(dots.iter().filter(|seat| seat.filled).count(), 3);
    assert_eq!(tally_label(3, 4), "3 / 4");
    assert_eq!(tally_tone(3, 4), "near");
    assert_eq!(tally_tone(1, 4), "far");
    assert_eq!(tally_note(3, 4), "3 approvals · 1 more for quorum");
    assert_eq!(tally_note(1, 4), "1 approval · 3 more for quorum");
    assert_eq!(tally_note(4, 4), "quorum met");
    assert_eq!(approve_label(3, 4), "Approve →");
    assert_eq!(approve_label(1, 4), "Approve");
}

#[test]
fn an_access_class_action_wears_the_brand_pair() {
    assert_eq!(proposal_kind_tone("add_validator"), "access");
    assert_eq!(proposal_kind_tone("signal"), "neutral");
}
