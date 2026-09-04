//! What the view asks the app for, as ordinary Ice tasks: the proposals
//! through one `query.governance`, the colour mode, and the refresh stream.
//! The reply is the module's own JSON — the same document the app's native
//! screen reads — shaped here into rows.

use iced::futures::{Stream, StreamExt};
use ui_lang_guest::host;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HostError {
    pub message: String,
}

impl From<String> for HostError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

/// One proposal, open or settled.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Proposal {
    pub id: String,
    pub open: bool,
    pub status: String,
    pub action: String,
    pub detail: String,
    pub approvals: i64,
    pub rejections: i64,
    pub required_yes: i64,
    pub electorate: i64,
    pub deadline: i64,
}

pub async fn load_proposals() -> Result<Vec<Proposal>, HostError> {
    let reply = host::request("query.governance", b"\"proposals\"").await?;
    parse_proposals(&reply)
}

/// The app's colour mode: `light` or `dark`, once on subscribing and again
/// on every change.
pub fn theme_changes() -> impl Stream<Item = Result<String, HostError>> + Send + 'static {
    host::theme().map(|answer| {
        answer
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .map_err(HostError::from)
    })
}

/// One item each time the app's generation for this module moves: the data
/// behind the view changed on the node.
pub fn refreshes() -> impl Stream<Item = Result<bool, HostError>> + Send + 'static {
    host::subscribe("host.refresh", &[]).map(|answer| answer.map(|_| true).map_err(HostError::from))
}

pub fn summary(rows: &[Proposal]) -> String {
    let open = rows.iter().filter(|row| row.open).count();
    let settled = rows.len() - open;
    format!("{open} open · {settled} settled")
}

pub fn tally(row: &Proposal) -> String {
    match row.open {
        true => format!(
            "{} of {} yes needed · {} no · {} eligible",
            row.approvals, row.required_yes, row.rejections, row.electorate
        ),
        false => format!("{} yes · {} no", row.approvals, row.rejections),
    }
}

pub fn status_label(row: &Proposal) -> String {
    match row.open {
        true => format!("open · until h {}", row.deadline),
        false => row.status.clone(),
    }
}

pub fn has_detail(row: &Proposal) -> bool {
    !row.detail.is_empty()
}

/// The module's `proposals` reply into rows, open first, newest deadline
/// first among them.
pub fn parse_proposals(reply: &[u8]) -> Result<Vec<Proposal>, HostError> {
    let reply: serde_json::Value = serde_json::from_slice(reply)
        .map_err(|error| HostError::from(format!("the governance reply is not JSON: {error}")))?;
    let mut rows: Vec<Proposal> = reply["proposals"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(proposal_row)
        .collect();
    rows.sort_by(|left, right| {
        right
            .open
            .cmp(&left.open)
            .then(right.deadline.cmp(&left.deadline))
    });
    Ok(rows)
}

fn proposal_row(view: &serde_json::Value) -> Proposal {
    let votes = view["votes"].as_array().cloned().unwrap_or_default();
    let approvals = votes
        .iter()
        .filter(|vote| vote[1].as_bool().unwrap_or(false))
        .count() as i64;
    let rejections = votes.len() as i64 - approvals;
    let status = tagged_name(&view["status"]);
    Proposal {
        id: view["proposal_id"].as_str().unwrap_or_default().to_string(),
        open: status == "open",
        detail: action_detail(&view["action"]),
        action: tagged_name(&view["action"]),
        deadline: view["deadline"].as_i64().unwrap_or(0),
        approvals,
        rejections,
        required_yes: yes_needed(&view["voting_rule"], rejections),
        electorate: view["electorate"]
            .as_array()
            .map_or(0, |members| members.len() as i64),
        status,
    }
}

/// A serde externally-tagged enum's variant name, or the string itself.
fn tagged_name(value: &serde_json::Value) -> String {
    value.as_str().map(str::to_string).unwrap_or_else(|| {
        value
            .as_object()
            .and_then(|tagged| tagged.keys().next().cloned())
            .unwrap_or_default()
    })
}

fn action_detail(action: &serde_json::Value) -> String {
    let Some((variant, payload)) = action.as_object().and_then(|tagged| tagged.iter().next())
    else {
        return String::new();
    };
    if let Some(text) = payload.get("text").and_then(|text| text.as_str()) {
        return text.to_string();
    }
    match variant.as_str() {
        "update_module" => format!(
            "{} → h {}",
            payload["name"].as_str().unwrap_or_default(),
            payload["activation_height"].as_i64().unwrap_or(0)
        ),
        "set_share_mode" => match payload["enabled"].as_bool().unwrap_or(false) {
            true => "account shares".into(),
            false => "one ballot per validator".into(),
        },
        _ => String::new(),
    }
}

/// How many YES votes pass this proposal at its current tally: a threshold
/// rule says so outright; a participating majority needs the quorum's
/// turnout and more yes than no.
fn yes_needed(rule: &serde_json::Value, rejections: i64) -> i64 {
    let Some((variant, payload)) = rule.as_object().and_then(|tagged| tagged.iter().next()) else {
        return 0;
    };
    match variant.as_str() {
        "participating_majority" => {
            let quorum = payload["quorum"].as_i64().unwrap_or(0);
            quorum.saturating_sub(rejections).max(rejections + 1)
        }
        _ => payload["required_yes"].as_i64().unwrap_or(0),
    }
}
