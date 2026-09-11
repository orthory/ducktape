//! The register the host pushes, the readings the screen folds off it, the
//! editor's draft arithmetic, and the intents a save leaves with.

use iced::futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use ui_lang_guest::host;

pub fn journal_width_after_delta(width: f64, delta: f64, viewport: f64) -> f64 {
    let maximum = (viewport - 10.0 - 320.0).clamp(280.0, 800.0);
    (width + delta).clamp(280.0, maximum)
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HostError {
    pub message: String,
}

/// One curated skill: a duckfs subtree, pinned at a snapshot or tracking the
/// committed head (an empty `source_snapshot`), loaded always (the persona)
/// or on demand.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSkill {
    pub name: String,
    pub source_prefix: String,
    pub source_snapshot: String,
    pub always: bool,
}

/// One registered agent — the whole record the desktop app read, with its
/// live-run fact and its controller.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRow {
    pub id: String,
    pub name: String,
    pub initials: String,
    pub capability: String,
    pub status: String,
    pub owner_handle: String,
    /// the controller's account number, decimal
    pub controller: String,
    pub live: bool,
    pub skills: Vec<AgentSkill>,
}

/// One run of an agent — a dispatch and what became of it — as the app
/// read it off the runs journal. Every stamp is pre-rendered: the view
/// owns no clock and no chain height.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRow {
    pub run_id: String,
    /// the run's address: what opens it, and what its journal is keyed by
    pub dispatch_id: String,
    pub agent_id: String,
    pub agent_name: String,
    /// what the run answers: a channel message, a job, or a calling run
    pub origin: String,
    /// `dispatched`, `running`, `accepted`, `rejected` or `failed`
    pub state: String,
    /// the dispatch height, rendered
    pub dispatched: String,
    /// the settlement height, rendered; "" while the run is in flight
    pub settled: String,
    pub attempt: i64,
    /// the executing node's key, abbreviated; "" before a session opened
    pub holder: String,
    pub actions: i64,
    pub degraded: bool,
    /// the failure excerpt of a failed run
    pub reason: String,
    pub output_ref: String,
    /// 0 when the run opened no PR
    pub pr_number: i64,
}

/// One journal entry of the run the reader opened.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    /// the commit height, rendered
    pub height: String,
    /// the fact's kind: `dispatched`, `session opened`, `acted`, `settled`,
    /// `result action refused`, `pr linked`
    pub kind: String,
    pub summary: String,
    pub status: String,
    pub targets: Vec<RunLink>,
}

/// One place a run touched, as a chip: where it was called from (`relation`
/// "from") or what its receipts landed on ("touched"). `url` is the duck://
/// address the chip opens; "" for a place the protocol has no address for
/// yet, which draws as a label alone.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunLink {
    pub relation: String,
    /// `chat`, `page`, `forge`, `file`, `task`, `job`, `module`,
    /// `conversation`, `run` or `output` — the glyph the chip wears
    pub kind: String,
    pub label: String,
    pub url: String,
}

/// The journal of one run, as the app last read it. `dispatch_id` names the
/// run it belongs to, so a journal that arrives after the reader moved on
/// is told apart from the one they are looking at.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunJournal {
    pub dispatch_id: String,
    pub entries: Vec<JournalEntry>,
    /// the run's origin and every place it touched, as chips
    pub links: Vec<RunLink>,
}

/// One step of a running agent: what it is doing, and whether it finished.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveActivity {
    pub label: String,
    pub done: bool,
}

/// The open run's progress off the node's live reading, while it runs:
/// absent (`present` false) for a run that settled or never opened here.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveRun {
    pub present: bool,
    pub status: String,
    pub activity: Vec<LiveActivity>,
    pub answer_preview: String,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentsProps {
    pub rows: Vec<AgentRow>,
    /// every agent's runs, newest dispatch first
    pub runs: Vec<RunRow>,
    /// the run the app has open for the reader, by dispatch id; "" is none.
    /// The app owns it because a run is opened from other tabs too — a chat
    /// hint, a bell, a duck://run link — and the panel follows.
    pub open_run: String,
    /// the doors the app has opened a run through, counted — a chat hint's
    /// "View run", a bell, a duck://run link, the runs list. The tracker is
    /// landed on at every bump, so a door onto the run already open still
    /// brings the reader to it from whichever panel they were on.
    pub opened: i64,
    /// the journal of the run the app has open for the reader
    pub journal: RunJournal,
    /// the open run's progress while it is still running
    pub live: LiveRun,
    /// every capability tag a node on the network announces
    pub capabilities: Vec<String>,
    /// the signing account's number, decimal; "" when the app has none
    pub account: String,
    /// bumped by the app on every committed agent write
    pub committed: i64,
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

/// `12 runs · 2 in flight` — the Runs panel's machine subtitle.
pub fn runs_summary(runs: &[RunRow]) -> String {
    if runs.is_empty() {
        return String::new();
    }
    let in_flight = runs
        .iter()
        .filter(|run| run.state == "dispatched" || run.state == "running")
        .count();
    let noun = if runs.len() == 1 { "run" } else { "runs" };
    format!("{} {noun} · {in_flight} in flight", runs.len())
}

/// The run listed under `run_id`; an empty row when the list has none.
pub fn run_named(runs: &[RunRow], run_id: &str) -> RunRow {
    runs.iter()
        .find(|run| run.run_id == run_id)
        .cloned()
        .unwrap_or_default()
}

/// The run listed under `dispatch_id`; an empty row when the list has none.
pub fn run_at(runs: &[RunRow], dispatch_id: &str) -> RunRow {
    runs.iter()
        .find(|run| run.dispatch_id == dispatch_id)
        .cloned()
        .unwrap_or_default()
}

pub fn empty_journal() -> RunJournal {
    RunJournal::default()
}

pub fn empty_live() -> LiveRun {
    LiveRun::default()
}

/// The glyph a chip wears for the kind of place it names.
pub fn link_glyph(kind: &str) -> String {
    match kind {
        "chat" => "#",
        "page" => "¶",
        "forge" => "⎇",
        "file" => "▤",
        "task" => "☐",
        "job" => "⚙",
        "module" => "⬡",
        "conversation" => "✉",
        "run" => "▶",
        "output" => "⇣",
        _ => "·",
    }
    .to_owned()
}

pub fn empty_run() -> RunRow {
    RunRow::default()
}

pub fn count_i64(count: usize) -> i64 {
    i64::try_from(count).unwrap_or(i64::MAX)
}

/// The row registered under `id`; an empty row when the register has none.
pub fn row_named(rows: &[AgentRow], id: &str) -> AgentRow {
    rows.iter()
        .find(|row| row.id == id)
        .cloned()
        .unwrap_or_default()
}

/// Whether the signing account may change this record: it is the record's
/// controller, and there is a network to write to.
pub fn editable(connected: bool, account: &str, controller: &str) -> bool {
    connected && !account.is_empty() && account == controller
}

pub fn has(list: &[String], item: &str) -> bool {
    list.iter().any(|entry| entry == item)
}

/// `skills` with the skill named `name` set to these fields — replaced in
/// place when it is already curated, appended otherwise. An empty name or
/// prefix adds nothing.
pub fn with_skill(
    skills: &[AgentSkill],
    name: &str,
    source_prefix: &str,
    source_snapshot: &str,
    always: bool,
) -> Vec<AgentSkill> {
    let name = name.trim();
    let source_prefix = source_prefix.trim();
    if name.is_empty() || source_prefix.is_empty() {
        return skills.to_vec();
    }
    let skill = AgentSkill {
        name: name.to_owned(),
        source_prefix: source_prefix.to_owned(),
        source_snapshot: source_snapshot.trim().to_owned(),
        always,
    };
    let mut next = skills.to_vec();
    match next.iter_mut().find(|existing| existing.name == name) {
        Some(existing) => *existing = skill,
        None => next.push(skill),
    }
    next
}

pub fn without_skill(skills: &[AgentSkill], name: &str) -> Vec<AgentSkill> {
    skills
        .iter()
        .filter(|skill| skill.name != name)
        .cloned()
        .collect()
}

/// `skills` with the named skill's load mode flipped to `always`.
pub fn skill_loaded(skills: &[AgentSkill], name: &str, always: bool) -> Vec<AgentSkill> {
    skills
        .iter()
        .map(|skill| {
            let mut next = skill.clone();
            if next.name == name {
                next.always = always;
            }
            next
        })
        .collect()
}

pub fn skill_mode(always: bool) -> String {
    if always { "always" } else { "on demand" }.to_owned()
}

/// The prefix a library skill named `name` lives under.
pub fn library_prefix(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        return String::new();
    }
    format!("/shared/skills/{name}")
}

/// What the capability picker offers: every announced tag, with the record's
/// current tag kept even when no node announces it today — a record must
/// not lose its executor by being opened.
pub fn capability_options(announced: &[String], current: &str) -> Vec<String> {
    let mut options = announced.to_vec();
    let current_unannounced = !current.is_empty() && !has(&options, current);
    if current_unannounced {
        options.insert(0, current.to_owned());
    }
    options
}

pub fn some_str(value: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }
    Some(value.to_owned())
}

pub fn skill_count(skills: &[AgentSkill]) -> i64 {
    count_i64(skills.len())
}

/// What the pane on screen IS, stated where a reader lands rather than
/// discovered by using it.
pub fn pane_note(pane: &str) -> String {
    match pane {
        "runs" => "",
        _ => {
            "The registry records who may act and as which account — every entry here is on \
             chain. The acting itself is recorded separately, as each agent's runs."
        }
    }
    .to_owned()
}

pub fn or_empty(value: &Option<String>) -> String {
    value.clone().unwrap_or_default()
}

pub fn pick_list(condition: bool, then: &[String], or: &[String]) -> Vec<String> {
    if condition { then } else { or }.to_vec()
}

pub fn pick_skills(condition: bool, then: &[AgentSkill], or: &[AgentSkill]) -> Vec<AgentSkill> {
    if condition { then } else { or }.to_vec()
}

/// One of two picks: `then` (empty meaning "nothing picked") when the
/// condition holds, the standing pick otherwise.
pub fn pick_option(condition: bool, then: &str, or: &Option<String>) -> Option<String> {
    if condition {
        return some_str(then);
    }
    or.clone()
}

/// The capability pick after a commit: the fresh row's tag when the drafts
/// were consumed, the reader's pick otherwise.
pub fn pick_capability(condition: bool, then: &str, or: &Option<String>) -> Option<String> {
    pick_option(condition, then, or)
}

pub fn pick_str(condition: bool, then: &str, or: &str) -> String {
    if condition { then } else { or }.to_owned()
}

/// An agent id the registry admits: a DNS label, lowercase `[a-z0-9-]`,
/// 1..=63 bytes, no hyphen at either end. The same rule the runs module
/// enforces, so the New agent form refuses before a program account is
/// minted for an id the registration would then reject.
pub fn valid_agent_id(id: &str) -> bool {
    let sized = !id.is_empty() && id.len() <= 63;
    let unhyphenated_ends = !id.starts_with('-') && !id.ends_with('-');
    let label_chars = id
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    sized && unhyphenated_ends && label_chars
}

// ---- the intents -----------------------------------------------------------

/// Pause (`paused: true`) or resume an agent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub agent_id: String,
    pub paused: bool,
}

/// The editor's whole record, as the app's `AgentDraft` decodes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    pub agent_id: String,
    pub display_name: String,
    pub capability: String,
    pub skills: Vec<AgentSkill>,
}

/// The run the reader opened, whose journal the app is asked to read; an
/// empty id closes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenRun {
    pub dispatch_id: String,
}

pub fn open_run(dispatch_id: &str) -> bool {
    notify(
        "agents.open_run",
        &OpenRun {
            dispatch_id: dispatch_id.into(),
        },
    )
}

/// A chip pressed: the duck:// address the app's open plane warps to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenLink {
    pub url: String,
}

pub fn open_link(url: &str) -> bool {
    notify("agents.open_link", &OpenLink { url: url.into() })
}

pub fn status(agent_id: &str, paused: bool) -> bool {
    notify(
        "agents.status",
        &Status {
            agent_id: agent_id.into(),
            paused,
        },
    )
}

fn draft(agent_id: &str, display_name: &str, capability: &str, skills: &[AgentSkill]) -> Draft {
    Draft {
        agent_id: agent_id.trim().to_owned(),
        display_name: display_name.trim().to_owned(),
        capability: capability.trim().to_owned(),
        skills: skills.to_vec(),
    }
}

/// Rewrite the record under `agent_id` with the draft.
pub fn save(agent_id: &str, display_name: &str, capability: &str, skills: &[AgentSkill]) -> bool {
    notify(
        "agents.save",
        &draft(agent_id, display_name, capability, skills),
    )
}

/// Register a new agent from the draft.
pub fn register(
    agent_id: &str,
    display_name: &str,
    capability: &str,
    skills: &[AgentSkill],
) -> bool {
    notify(
        "agents.register",
        &draft(agent_id, display_name, capability, skills),
    )
}

fn notify<T: Serialize>(operation: &str, payload: &T) -> bool {
    let bytes = serde_json::to_vec(payload).expect("an intent encodes");
    host::notify(operation, &bytes);
    true
}
