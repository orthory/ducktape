//! What the view asks of the host, and the readings the screen folds off a
//! proposal register. The register itself arrives from the host as JSON —
//! ordinary record data crossing as values — and leaves as intents nobody
//! waits on: the host performs the write and pushes the next register.

use iced::futures::StreamExt;
use serde::{Deserialize, Serialize};
use ui_lang_guest::host;

/// One governance proposal, rendered by the host: the same fields the
/// desktop app's own register carries, and nothing derived.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct ProposalRow {
    pub id: String,
    pub action: String,
    pub detail: String,
    pub proposer: String,
    pub status: String,
    pub deadline: i64,
    pub approvals: i64,
    pub rejections: i64,
    pub rule: String,
    pub required_yes: i64,
    pub electorate: i64,
    pub open: bool,
    pub settled_height: i64,
}

/// Everything the screen shows, as the host has it right now.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct GovernanceProps {
    pub rows: Vec<ProposalRow>,
    /// The proposal a write is in flight for, or empty.
    pub voting: String,
    pub admin: bool,
    pub connected: bool,
    pub answered: bool,
    pub dark: bool,
}

/// One item of the register subscription: the register, or why not.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct PropsItem {
    pub next: GovernanceProps,
    pub error: String,
}

/// The register now, and again on every change the host sees.
pub fn props() -> iced::Subscription<PropsItem> {
    iced::Subscription::run(|| {
        host::subscribe("governance.props", &[]).map(|answer| {
            let read = answer.and_then(|bytes| {
                serde_json::from_slice(&bytes).map_err(|error| error.to_string())
            });
            match read {
                Ok(next) => PropsItem {
                    next,
                    error: String::new(),
                },
                Err(error) => PropsItem {
                    next: GovernanceProps::default(),
                    error,
                },
            }
        })
    })
}

/// A vote or a settle, as the host hears it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Intent {
    pub proposal_id: String,
    pub approve: bool,
}

/// Casts a vote. Nobody waits: the host answers with the next register.
pub fn vote(proposal_id: String, approve: bool) -> bool {
    intend("governance.vote", proposal_id, approve)
}

/// Executes a proposal whose rule is met.
pub fn execute(proposal_id: String) -> bool {
    intend("governance.execute", proposal_id, true)
}

fn intend(kind: &str, proposal_id: String, approve: bool) -> bool {
    let intent = Intent {
        proposal_id,
        approve,
    };
    let payload = serde_json::to_vec(&intent).expect("an intent encodes");
    host::notify(kind, &payload);
    true
}

// ---------- the readings ----------

/// `12 open · 3 settled` — the Approvals title's machine subtitle.
pub fn proposals_summary(connected: bool, rows: &[ProposalRow]) -> String {
    if !connected || rows.is_empty() {
        return String::new();
    }
    let open = rows.iter().filter(|row| row.open).count();
    format!("{open} open · {} settled", rows.len() - open)
}

/// `N pending` — the header count, open proposals only.
pub fn pending_label(rows: &[ProposalRow]) -> String {
    format!("{} pending", rows.iter().filter(|row| row.open).count())
}

/// How many proposals are still open.
pub fn open_proposals(rows: &[ProposalRow]) -> i64 {
    rows.iter().filter(|row| row.open).count() as i64
}

/// The settled half of the register — the RECENTLY FINALIZED column.
pub fn settled_proposals(rows: &[ProposalRow]) -> Vec<ProposalRow> {
    rows.iter().filter(|row| !row.open).cloned().collect()
}

/// One seat per REQUIRED signature, filled for each approval already in —
/// the quorum dots. Capped so a large threshold does not overflow the card.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct QuorumSeat {
    pub filled: bool,
}

pub fn quorum_dots(approvals: i64, required: i64) -> Vec<QuorumSeat> {
    let seats = required.clamp(0, 12) as usize;
    (0..seats)
        .map(|seat| QuorumSeat {
            filled: (seat as i64) < approvals,
        })
        .collect()
}

/// `3 / 4` — the tally, one mono run.
pub fn tally_label(approvals: i64, required: i64) -> String {
    format!("{approvals} / {required}")
}

/// `near` one vote from quorum (or past it), else `far` — success vs meta ink.
pub fn tally_tone(approvals: i64, required: i64) -> String {
    match approvals >= required.saturating_sub(1) {
        true => "near".into(),
        false => "far".into(),
    }
}

/// `3 approvals · 1 more for quorum`, or `quorum met`.
pub fn tally_note(approvals: i64, required: i64) -> String {
    let remaining = required.saturating_sub(approvals);
    if remaining <= 0 {
        return "quorum met".into();
    }
    let have = plural(approvals, "approval", "approvals");
    format!("{have} · {remaining} more for quorum")
}

/// The approve button leans forward at the last vote: `Approve →`.
pub fn approve_label(approvals: i64, required: i64) -> String {
    match approvals + 1 >= required {
        true => "Approve →".into(),
        false => "Approve".into(),
    }
}

/// The kind pill's two tones: an access-class action reads `access`.
pub fn proposal_kind_tone(action: &str) -> String {
    let access = matches!(
        action,
        "add_validator" | "add_resident" | "remove_validator" | "remove_resident" | "grant_client"
    );
    match access {
        true => "access".into(),
        false => "neutral".into(),
    }
}

/// `h 84,912` — a block height, grouped; a negative one is `h —`.
pub fn height_label_short(height: i64) -> String {
    if height < 0 {
        return "h —".into();
    }
    let digits = height.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        let group_boundary = index > 0 && (digits.len() - index).is_multiple_of(3);
        if group_boundary {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    format!("h {grouped}")
}

fn plural(count: i64, one: &str, many: &str) -> String {
    let noun = if count == 1 { one } else { many };
    format!("{count} {noun}")
}
