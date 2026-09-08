//! The facts the host pushes, and the writes that leave as intents — one
//! per act the shell screen offers. The words of a task never cross here:
//! the composer is the host's surface, and a send is the host's intent.

use iced::futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use ui_lang_guest::host;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HostError {
    pub message: String,
}

/// One step of a provider turn, as the backend caps and titles it.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct AgentActivity {
    pub id: i64,
    pub title: String,
    pub detail: String,
    pub status: String,
}

/// One transcript turn, with the provider's label and initial and the run
/// label already folded by the host — the view names no provider itself.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct AgentChatEntry {
    pub id: i64,
    pub role: String,
    pub body: String,
    pub provider_label: String,
    pub provider_initial: String,
    pub status: String,
    pub run_label: String,
    pub steps: Vec<AgentActivity>,
    pub steps_label: String,
}

/// The screen's facts, as the app holds them. `surface` is `tasks` or
/// `terminal`; the six lines of prose (`run_line`, `grant_note`,
/// `terminal_note`, `composer_hint`, `task_blurb`, `register_hint`) are the
/// app's wording over its own picks.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct ShellProps {
    pub dark: bool,
    pub connected: bool,
    pub surface: String,
    pub setup_open: bool,
    pub identity_options: Vec<String>,
    pub identity: String,
    pub provider_initial: String,
    pub credential: String,
    pub host_node_options: Vec<String>,
    pub host_node: String,
    pub credentials_loading: bool,
    pub terminal_running: bool,
    pub terminal_busy: bool,
    pub terminal_title: String,
    pub terminal_error: String,
    pub entries: Vec<AgentChatEntry>,
    pub activity: Vec<AgentActivity>,
    pub chat_busy: bool,
    pub chat_status: String,
    pub chat_detail: String,
    pub live: String,
    pub saga_id: String,
    pub detached_saga: String,
    pub run_line: String,
    pub grant_note: String,
    pub terminal_note: String,
    pub composer_hint: String,
    pub task_blurb: String,
    pub register_hint: String,
}

/// The facts now, and again on every change the host sees.
pub fn props() -> impl Stream<Item = Result<ShellProps, HostError>> + Send + 'static {
    host::subscribe("shell.props", &[]).map(|answer| {
        let bytes = answer.map_err(|message| HostError { message })?;
        serde_json::from_slice(&bytes).map_err(|error| HostError {
            message: error.to_string(),
        })
    })
}

/// `shell.surface` — show the tasks or the terminal surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Surface {
    pub surface: String,
}

/// `shell.identity` / `shell.host_node` — the pick the setup settled on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pick {
    pub value: String,
}

/// `shell.open_link` — a link activated in an answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub url: String,
}

fn notify<T: Serialize>(kind: &str, payload: &T) -> bool {
    host::notify(kind, &serde_json::to_vec(payload).expect("intent encode"));
    true
}

pub fn show_surface(surface: &str) -> bool {
    notify(
        "shell.surface",
        &Surface {
            surface: surface.into(),
        },
    )
}

pub fn toggle_setup() -> bool {
    notify("shell.setup", &())
}

pub fn pick_identity(value: &str) -> bool {
    notify(
        "shell.identity",
        &Pick {
            value: value.into(),
        },
    )
}

pub fn pick_host_node(value: &str) -> bool {
    notify(
        "shell.host_node",
        &Pick {
            value: value.into(),
        },
    )
}

pub fn refresh_credentials() -> bool {
    notify("shell.refresh", &())
}

pub fn start_terminal() -> bool {
    notify("shell.terminal_start", &())
}

pub fn stop_terminal() -> bool {
    notify("shell.terminal_stop", &())
}

pub fn reset_chat() -> bool {
    notify("shell.reset", &())
}

pub fn detach_run() -> bool {
    notify("shell.detach", &())
}

pub fn reopen_run() -> bool {
    notify("shell.reopen", &())
}

pub fn discard_run() -> bool {
    notify("shell.discard", &())
}

pub fn open_link(url: &str) -> bool {
    notify("shell.open_link", &Link { url: url.into() })
}

/// The product icon set, as bytes the wire carries.
pub fn icon(name: &str) -> Vec<u8> {
    design::icons::svg(name).as_bytes().to_vec()
}

/// Which settled turn has its work open after a fold click: the same id
/// closes it, another opens that one.
pub fn toggle_fold(open: i64, id: i64) -> i64 {
    if open == id { 0 } else { id }
}
