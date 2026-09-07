//! The register the host pushes, and the one reading the screen folds off it.

use iced::futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use ui_lang_guest::host;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HostError {
    pub message: String,
}

/// One registered agent, rendered by the host from its registry record and
/// live-run fact — the same fields the desktop app's own register carries.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct AgentRow {
    pub id: String,
    pub name: String,
    pub initials: String,
    pub capability: String,
    pub status: String,
    pub owner_handle: String,
    pub live: bool,
    pub skill_count: i64,
    pub cap_count: i64,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct AgentsProps {
    pub rows: Vec<AgentRow>,
    pub connected: bool,
    pub answered: bool,
    pub dark: bool,
}

/// The register now, and again on every change the host sees.
pub fn props() -> impl Stream<Item = Result<AgentsProps, HostError>> + Send + 'static {
    host::subscribe("agents.props", &[]).map(|answer| {
        let bytes = answer.map_err(|message| HostError { message })?;
        serde_json::from_slice(&bytes).map_err(|error| HostError {
            message: error.to_string(),
        })
    })
}

/// `4 agents · 2 working` — the title's machine subtitle. `working` is runs
/// in flight, not `status == active`, which is the registration default.
pub fn agents_summary(connected: bool, rows: &[AgentRow]) -> String {
    if !connected || rows.is_empty() {
        return String::new();
    }
    let working = rows.iter().filter(|row| row.live).count();
    let noun = if rows.len() == 1 { "agent" } else { "agents" };
    format!("{} {noun} · {working} working", rows.len())
}
