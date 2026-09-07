//! The roster the host pushes, the readings folded off it, and the three
//! writes that leave as intents.

use iced::futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use ui_lang_guest::host;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HostError {
    pub message: String,
}

/// One member of the network — the same fields the desktop app's roster row
/// carries.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct MemberRow {
    pub key: String,
    pub label: String,
    pub role: String,
    pub is_this_node: bool,
    pub is_agent: bool,
    pub model: String,
    pub live: bool,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct MembersProps {
    pub rows: Vec<MemberRow>,
    pub admin: bool,
    pub connected: bool,
    pub answered: bool,
    pub dark: bool,
}

/// The roster now, and again on every change the host sees.
pub fn props() -> impl Stream<Item = Result<MembersProps, HostError>> + Send + 'static {
    host::subscribe("members.props", &[]).map(|answer| {
        let bytes = answer.map_err(|message| HostError { message })?;
        serde_json::from_slice(&bytes).map_err(|error| HostError {
            message: error.to_string(),
        })
    })
}

/// `members.copy` — the host puts `text` on the clipboard and toasts `label`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Copy {
    pub text: String,
    pub label: String,
}

/// `members.agent_status` — the DESIRED paused state of `agent_id`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentStatus {
    pub agent_id: String,
    pub paused: bool,
}

/// `members.propose` — open a membership ballot: `add_validator` or
/// `remove_validator` over `key`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Propose {
    pub action: String,
    pub key: String,
}

pub fn copy(text: &str, label: &str) -> bool {
    notify(
        "members.copy",
        &Copy {
            text: text.into(),
            label: label.into(),
        },
    )
}

pub fn agent_status(agent_id: &str, paused: bool) -> bool {
    notify(
        "members.agent_status",
        &AgentStatus {
            agent_id: agent_id.into(),
            paused,
        },
    )
}

pub fn propose(action: &str, key: &str) -> bool {
    notify(
        "members.propose",
        &Propose {
            action: action.into(),
            key: key.into(),
        },
    )
}

fn notify<T: Serialize>(operation: &str, payload: &T) -> bool {
    let bytes = serde_json::to_vec(payload).expect("an intent encodes");
    host::notify(operation, &bytes);
    true
}

/// `2 humans · 1 agent` — the title's machine subtitle, folded off the same
/// rows the filter strip splits.
pub fn members_summary(connected: bool, rows: &[MemberRow]) -> String {
    if !connected || rows.is_empty() {
        return String::new();
    }
    let agents = rows.iter().filter(|row| row.is_agent).count();
    let left = plural(rows.len() - agents, "human", "humans");
    let right = plural(agents, "agent", "agents");
    format!("{left} · {right}")
}

/// The All / Humans / Agents / Validators strip.
pub(crate) fn filter_members(rows: &[MemberRow], filter: crate::MembersFilter) -> Vec<MemberRow> {
    rows.iter()
        .filter(|row| match filter {
            crate::MembersFilter::All => true,
            crate::MembersFilter::Humans => !row.is_agent,
            crate::MembersFilter::Agents => row.is_agent,
            crate::MembersFilter::Validators => row.role == "validator",
        })
        .cloned()
        .collect()
}

/// Two letters for a machine principal: the first of each of two words, else
/// the first two alphanumerics.
pub fn initials_of(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().take(2).collect();
    if words.len() == 2 {
        let letters: String = words
            .iter()
            .filter_map(|word| word.chars().find(char::is_ascii_alphanumeric))
            .collect();
        if letters.chars().count() == 2 {
            return letters.to_uppercase();
        }
    }
    let letters: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(2)
        .collect();
    match letters.is_empty() {
        true => "?".into(),
        false => letters.to_uppercase(),
    }
}

/// One letter for a person.
pub fn initial_of(name: &str) -> String {
    name.trim()
        .chars()
        .next()
        .map(|first| first.to_uppercase().to_string())
        .unwrap_or_default()
}

fn plural(count: usize, one: &str, many: &str) -> String {
    let noun = if count == 1 { one } else { many };
    format!("{count} {noun}")
}
