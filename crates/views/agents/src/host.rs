//! The register the host pushes, the readings the screen folds off it, the
//! editor's draft arithmetic, and the intents a save leaves with.

use iced::futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use ui_lang_guest::host;

/// Presentation only: routing and the expandable receipt retain the full value.
pub fn compact_run_text(value: &str) -> String {
    value
        .split(' ')
        .map(|word| {
            let is_long = word.chars().count() > 32;
            if is_long {
                format!("{}…", word.chars().take(16).collect::<String>())
            } else {
                word.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn journal_summary(kind: &str, summary: &str) -> String {
    if kind == "acted" {
        let operation = summary.split(" · ").next().unwrap_or(summary);
        // Acted records admission to the action queue, not the target's completion.
        let action = match operation {
            "react" => "Reaction requested".to_owned(),
            "unreact" => "Reaction removal requested".to_owned(),
            "reply" => "Reply requested".to_owned(),
            "agent.call" => "Agent call requested".to_owned(),
            other => format!("Requested: {other}"),
        };
        return format!("→ {action}");
    }
    compact_run_text(summary)
}

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

/// The resource grant, list by list. Every list is a set the editor adds to
/// and removes from; the budget is the concurrent peer-call ceiling.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCaps {
    pub forge_read: Vec<String>,
    pub forge_push: Vec<String>,
    pub duckfs_read: Vec<String>,
    pub duckfs_write: Vec<String>,
    pub tools: Vec<String>,
    pub secrets: Vec<String>,
    pub pages_write: Vec<String>,
    pub subagent_budget: i64,
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
    pub allowed_actions: Vec<String>,
    pub caps: AgentCaps,
    pub skills: Vec<AgentSkill>,
}

/// One seat on the open conversation's roster — the participant/visibility
/// disclosure the spec requires beside every message.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessagingSeat {
    pub participant: String,
    /// `member` (may send) or `observer` (subscribes without input authority)
    pub role: String,
    /// this seat is the participant the viewer is acting as
    pub you: bool,
}

/// The viewer's own input binding on the open conversation, as `BindingView`
/// exposes it. The scoped service key is NOT part of that record by
/// construction, and no socket path, session id or provider token is either.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessagingBinding {
    pub present: bool,
    /// the operator-chosen device label
    pub device: String,
    /// the binding's credential number, decimal
    pub credential: String,
    /// `service_key` (a local adapter's owner-issued scoped key, reported by
    /// SHAPE — the key itself is never read back) or `program` (an agent
    /// program reaching the module over the call lane). "" when unbound.
    pub principal: String,
    /// the bound program's account number, decimal; "" for a service key
    pub principal_account: String,
    pub detached: bool,
}

/// One admitted message as the app projected it: the immutable record, its
/// SEPARATE per-recipient delivery state, and the display bounds the app
/// applied. `body` may be a bounded slice of the stored body — `body_bytes`
/// against `shown_bytes` says so, and the stored message is never altered.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessagingMessage {
    /// the conversation event sequence this admission holds
    pub seq: i64,
    pub sender: String,
    pub recipient: String,
    /// notice | question | task_request | task_update | result
    pub kind: String,
    pub body: String,
    pub body_bytes: i64,
    pub shown_bytes: i64,
    /// the immutable references, rendered and bounded by the app
    pub references: String,
    /// the causal parent's seq, 0 when the message answers nothing
    pub reply_to: i64,
    /// the referenced task's id, "" when the message names none
    pub task: String,
    /// the attempt the sender addressed
    pub task_attempt: i64,
    /// stored | queued | adapter_accepted | held | refused | expired |
    /// delivery_unknown — folded from the committed event stream
    pub delivery: String,
    /// the stable snake_case reason token, "" when the record carries none
    pub delivery_reason: String,
    pub mine: bool,
    pub expires_at: i64,
    pub admitted_at: i64,
}

/// The conversation the viewer explicitly opened, everything the app could
/// authenticate about it, and nothing it could not.
///
/// Every field is the answer to ONE authenticated read as the acting
/// participant. A refusal is [`Self::denied`] or [`Self::error`] — never an
/// empty [`Self::messages`], which would read as "this conversation is empty"
/// for a conversation nobody was allowed to see.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessagingProps {
    /// the participant the viewer is acting as; "" while none is chosen
    pub participant: String,
    /// the conversation open in the panel; "" while none is
    pub conversation: String,
    /// the chain the panel's contents were read from — the immutable network
    /// identity every id below is scoped by
    pub network: String,
    pub topic: String,
    pub roster: Vec<MessagingSeat>,
    pub binding: MessagingBinding,
    pub messages: Vec<MessagingMessage>,
    pub may_read: bool,
    pub may_send: bool,
    /// the module's own refusal token: unauthenticated | not_reader |
    /// not_permitted. "" when the read was answered.
    pub denied: String,
    /// a transport or module failure, as one sentence. "" when there is none.
    pub error: String,
    /// the page's cursor was below the retained floor: resync, never advance
    pub history_gap: bool,
    pub floor_seq: i64,
    pub from_seq: i64,
    /// the conversation's next committed sequence — its live tip
    pub next_seq: i64,
    /// events one page asks the network for — the step "older" and "newer"
    /// move by, so the guest never guesses the app's page size
    pub page_size: i64,
    pub more_before: bool,
    pub more_after: bool,
    pub undelivered: i64,
    pub queued_bytes: i64,
    pub max_body_bytes: i64,
    pub loading: bool,
    /// a read for THIS scope has answered at least once
    pub answered: bool,
    pub sending: bool,
    /// the last send's refusal, as one sentence; the draft is kept
    pub send_error: String,
    /// the conversation sequence the last accepted send landed at, 0 for none
    pub sent_seq: i64,
    /// what this network's storage/read visibility actually is
    pub visibility: String,
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
    /// the action vocabulary a grant draws from
    pub actions: Vec<String>,
    /// the signing account's number, decimal; "" when the app has none
    pub account: String,
    /// bumped by the app on every committed agent write
    pub committed: i64,
    pub connected: bool,
    pub answered: bool,
    pub dark: bool,
    /// the messaging panel's whole reading
    pub messaging: MessagingProps,
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

/// How many grants a record carries: every cap list, and the budget once
/// when it admits anyone.
pub fn cap_count(caps: &AgentCaps) -> i64 {
    let count = caps.forge_read.len()
        + caps.forge_push.len()
        + caps.duckfs_read.len()
        + caps.duckfs_write.len()
        + caps.tools.len()
        + caps.secrets.len()
        + caps.pages_write.len()
        + usize::from(caps.subagent_budget > 0);
    count_i64(count)
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

/// `list` with `item` present or absent as `on` says, kept sorted and
/// deduped — the shape the registry canonicalizes to anyway.
pub fn with_flag(list: &[String], item: &str, on: bool) -> Vec<String> {
    if on {
        with_entry(list, item)
    } else {
        without(list, item)
    }
}

/// `list` plus `entry`, trimmed; an empty entry adds nothing.
pub fn with_entry(list: &[String], entry: &str) -> Vec<String> {
    let entry = entry.trim();
    if entry.is_empty() {
        return list.to_vec();
    }
    let mut next = list.to_vec();
    next.push(entry.to_owned());
    next.sort();
    next.dedup();
    next
}

pub fn without(list: &[String], entry: &str) -> Vec<String> {
    list.iter().filter(|item| *item != entry).cloned().collect()
}

/// The cap lists, named the way the record names them, for the editor's
/// "add a grant" kind picker.
pub const CAP_KINDS: [&str; 7] = [
    "forge_read",
    "forge_push",
    "duckfs_read",
    "duckfs_write",
    "tools",
    "secrets",
    "pages_write",
];

pub fn cap_kinds() -> Vec<String> {
    CAP_KINDS.iter().map(|kind| (*kind).to_owned()).collect()
}

fn cap_list_mut<'a>(caps: &'a mut AgentCaps, kind: &str) -> Option<&'a mut Vec<String>> {
    match kind {
        "forge_read" => Some(&mut caps.forge_read),
        "forge_push" => Some(&mut caps.forge_push),
        "duckfs_read" => Some(&mut caps.duckfs_read),
        "duckfs_write" => Some(&mut caps.duckfs_write),
        "tools" => Some(&mut caps.tools),
        "secrets" => Some(&mut caps.secrets),
        "pages_write" => Some(&mut caps.pages_write),
        _ => None,
    }
}

/// `caps` with `entry` added to the `kind` list.
pub fn caps_with(caps: &AgentCaps, kind: &str, entry: &str) -> AgentCaps {
    let mut next = caps.clone();
    if let Some(list) = cap_list_mut(&mut next, kind) {
        *list = with_entry(list, entry);
    }
    next
}

/// `caps` with `entry` removed from the `kind` list.
pub fn caps_without(caps: &AgentCaps, kind: &str, entry: &str) -> AgentCaps {
    let mut next = caps.clone();
    if let Some(list) = cap_list_mut(&mut next, kind) {
        *list = without(list, entry);
    }
    next
}

/// `caps` with the budget the reader typed; text that is not a whole number
/// reads as no budget.
pub fn caps_with_budget(caps: &AgentCaps, text: &str) -> AgentCaps {
    let mut next = caps.clone();
    next.subagent_budget = text.trim().parse().unwrap_or(0);
    next
}

pub fn budget_text(budget: i64) -> String {
    if budget == 0 {
        return String::new();
    }
    budget.to_string()
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

/// The empty grant: the record's default, and the New form's start.
pub fn empty_caps() -> AgentCaps {
    AgentCaps::default()
}

/// The messaging panel before any read has answered: nothing open, nothing
/// claimed about a conversation nobody asked for.
pub fn empty_messaging() -> MessagingProps {
    MessagingProps::default()
}

/// What the pane on screen IS, stated where a reader lands rather than
/// discovered by using it. The messages sentence is the one this whole panel
/// exists not to blur: a delivery state is about an input interface, and
/// nothing here reads it as work.
pub fn pane_note(pane: &str) -> String {
    match pane {
        "messages" => {
            "Messages are immutable records with a separate delivery state per recipient. A \
             delivery state says what a provider's input interface did with an input — never \
             that a model read it, understood it, acted on it, or that a task was claimed or \
             finished. Runs is where execution is settled."
        }
        "runs" => {
            "A run is a dispatch and what the network settled about it: who held it, what it \
             acted on, and how it ended. This is the only execution status on this screen — a \
             message's delivery state is not one."
        }
        _ => {
            "The registry records who may act, what they may do, and under whose grant — every \
             entry here is on chain. The acting itself is recorded separately, as each agent's \
             runs."
        }
    }
    .to_owned()
}

pub fn pick_int(condition: bool, then: i64, or: i64) -> i64 {
    if condition { then } else { or }
}

pub fn or_empty(value: &Option<String>) -> String {
    value.clone().unwrap_or_default()
}

pub fn pick_list(condition: bool, then: &[String], or: &[String]) -> Vec<String> {
    if condition { then } else { or }.to_vec()
}

pub fn pick_caps(condition: bool, then: &AgentCaps, or: &AgentCaps) -> AgentCaps {
    if condition { then } else { or }.clone()
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
    pub allowed_actions: Vec<String>,
    pub caps: AgentCaps,
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

fn draft(
    agent_id: &str,
    display_name: &str,
    capability: &str,
    allowed_actions: &[String],
    caps: &AgentCaps,
    skills: &[AgentSkill],
) -> Draft {
    Draft {
        agent_id: agent_id.trim().to_owned(),
        display_name: display_name.trim().to_owned(),
        capability: capability.trim().to_owned(),
        allowed_actions: allowed_actions.to_vec(),
        caps: caps.clone(),
        skills: skills.to_vec(),
    }
}

/// Rewrite the record under `agent_id` with the draft.
pub fn save(
    agent_id: &str,
    display_name: &str,
    capability: &str,
    allowed_actions: &[String],
    caps: &AgentCaps,
    skills: &[AgentSkill],
) -> bool {
    notify(
        "agents.save",
        &draft(
            agent_id,
            display_name,
            capability,
            allowed_actions,
            caps,
            skills,
        ),
    )
}

/// Register a new agent from the draft.
pub fn register(
    agent_id: &str,
    display_name: &str,
    capability: &str,
    allowed_actions: &[String],
    caps: &AgentCaps,
    skills: &[AgentSkill],
) -> bool {
    notify(
        "agents.register",
        &draft(
            agent_id,
            display_name,
            capability,
            allowed_actions,
            caps,
            skills,
        ),
    )
}

fn notify<T: Serialize>(operation: &str, payload: &T) -> bool {
    let bytes = serde_json::to_vec(payload).expect("an intent encodes");
    host::notify(operation, &bytes);
    true
}

// ---- the messaging panel's readings ----------------------------------------

/// The delivery chip's word. Deliberately not the state token: the token is
/// the machine contract, this is what a person reads.
pub fn delivery_label(state: &str) -> String {
    match state {
        "stored" => "STORED",
        "queued" => "QUEUED",
        "adapter_accepted" => "ACCEPTED",
        "held" => "HELD",
        "refused" => "REFUSED",
        "expired" => "EXPIRED",
        "delivery_unknown" => "UNKNOWN",
        _ => "UNKNOWN",
    }
    .to_owned()
}

/// What that state DOES and DOES NOT say. The one place this screen is at risk
/// of implying that a model read something, so each sentence names the limit.
pub fn delivery_note(state: &str) -> String {
    match state {
        "stored" => "The network accepted this immutable record. No service has queued it yet.",
        "queued" => "The bound service has durably queued it. The provider has not been given it.",
        "adapter_accepted" => {
            "The provider's input interface accepted it — not that the model read it, understood \
             it, acted on it, or claimed a task."
        }
        "held" => {
            "A provider or local approval barrier is holding it. This view does not override that."
        }
        "refused" => "Delivery was refused.",
        "expired" => {
            "The delivery deadline passed. Expiry stops further delivery; it does not undo work \
             already accepted."
        }
        "delivery_unknown" => {
            "The service cannot establish whether the provider accepted this input. It is not \
             replayed automatically."
        }
        _ => "This app does not know this delivery state.",
    }
    .to_owned()
}

/// A delivery state whose plate warns rather than reassures.
///
/// Named by what it is NOT: the three states that are ordinary progress. A
/// token this screen does not know — including the empty one the app leaves
/// when it could not read the receipt — warns, because the alternative is a
/// reassuring plate under a state nobody established.
pub fn delivery_unsettled(state: &str) -> bool {
    !matches!(state, "stored" | "queued" | "adapter_accepted")
}

pub fn kind_label(kind: &str) -> String {
    match kind {
        "notice" => "notice",
        "question" => "question",
        "task_request" => "task request",
        "task_update" => "task update",
        "result" => "result",
        _ => kind,
    }
    .to_owned()
}

/// What the panel is showing of a body it did not alter. Empty while the whole
/// body is on screen.
pub fn body_note(body_bytes: i64, shown_bytes: i64) -> String {
    if shown_bytes >= body_bytes {
        return String::new();
    }
    format!("showing {shown_bytes} of {body_bytes} bytes — the stored message is unchanged")
}

/// A message's task reference, and the limit on what this screen knows about
/// it. Delivery is not work: nothing here resolves whether the task was
/// claimed, ran, or finished, and no prose or terminal output is read as if it
/// did.
pub fn task_note(task: &str, attempt: i64) -> String {
    if task.is_empty() {
        return String::new();
    }
    format!("task {task} · attempt {attempt} · execution status not resolved here")
}

/// The run this message's task reference actually names, or "" — checked
/// against the runs journal on this same screen, never derived from the id's
/// shape.
///
/// This is the ONLY link from a message to an execution status, and it fails
/// closed: a task id that is not a run in the journal gets no link and keeps
/// [`task_note`]'s "not resolved here". A run id is a different identity from
/// a task id, and pretending one is the other would put a settled state under
/// a message that never had one.
pub fn run_for_task(runs: &[RunRow], task: &str) -> String {
    if task.is_empty() {
        return String::new();
    }
    runs.iter()
        .find(|run| run.run_id == task)
        .map(|run| run.run_id.clone())
        .unwrap_or_default()
}

/// The label of the link above — the run's own state named, so pressing it is
/// a choice rather than a guess.
pub fn run_link_note(runs: &[RunRow], task: &str) -> String {
    let Some(run) = runs.iter().find(|run| run.run_id == task) else {
        return String::new();
    };
    format!("open run {} · {}", run.run_id, run.state)
}

/// Why an authenticated read was refused, in the module's own vocabulary.
pub fn denied_note(denied: &str) -> String {
    match denied {
        "unauthenticated" => {
            "This read carried no authenticated caller. Unlock this device's key and try again."
        }
        "not_reader" => {
            "This device's key is neither this participant's owner nor its bound service key."
        }
        "not_permitted" => "This participant is revoked, or is not on this conversation's roster.",
        "" => "",
        _ => "The network refused this read.",
    }
    .to_owned()
}

pub fn seat_role(role: &str) -> String {
    match role {
        "member" => "member · may send",
        "observer" => "observer · subscribes, does not send",
        _ => role,
    }
    .to_owned()
}

/// The viewer's input binding, stated whole — including holding none, which is
/// the difference between "queued for my device" and "queued for nobody".
pub fn binding_note(binding: &MessagingBinding) -> String {
    if !binding.present {
        return "No input binding on this conversation from this participant. Messages are stored \
                and stay undelivered until a device attaches."
            .to_owned();
    }
    let device = &binding.device;
    let credential = &binding.credential;
    if binding.detached {
        return format!(
            "Binding on {device} is detached; credential {credential} is spent and can neither \
             send nor acknowledge."
        );
    }
    format!(
        "Bound to {device} under credential {credential}, authorizing {}.",
        principal_note(binding)
    )
}

/// WHO the binding authorizes, spelled out. A device label says which machine;
/// this says which kind of principal may act under the credential, which is a
/// different question and the one that matters — an agent PROGRAM attached to
/// this participant is not the same disclosure as a person's local adapter,
/// and the label cannot tell them apart.
pub fn principal_note(binding: &MessagingBinding) -> String {
    match binding.principal.as_str() {
        "service_key" => {
            "a scoped service key on that device (the key itself is a credential this view never \
             reads back)"
                .to_owned()
        }
        "program" => format!(
            "agent program account {} over the call lane",
            binding.principal_account
        ),
        // the app leaves this empty only when it could not resolve the record;
        // an unnamed principal is stated as unknown, never assumed benign
        _ => "a principal this view could not identify".to_owned(),
    }
}

pub fn mailbox_note(undelivered: i64, queued_bytes: i64) -> String {
    format!("{undelivered} undelivered · {queued_bytes} bytes queued")
}

/// Which sequences the page on screen covers, against the conversation's live
/// tip — so a reader can see there IS a tail rather than guess.
pub fn page_note(messages: &[MessagingMessage], from_seq: i64, next_seq: i64) -> String {
    let Some(first) = messages.first() else {
        return format!("no messages from sequence {from_seq}; the conversation is at {next_seq}");
    };
    let last = messages.last().map_or(first.seq, |message| message.seq);
    format!(
        "{} shown · sequences {}–{last} of {next_seq}",
        messages.len(),
        first.seq
    )
}

/// The kinds a PERSON may compose here. `task_update` is absent on purpose: it
/// must name a task and the attempt it addresses, which an attached service
/// holds and this panel does not. `result` needs a task or a message it
/// answers, so it appears only while replying.
pub fn compose_kinds(reply_to: i64) -> Vec<String> {
    let mut kinds = vec![
        "notice".to_owned(),
        "question".to_owned(),
        "task_request".to_owned(),
    ];
    if reply_to > 0 {
        kinds.push("result".to_owned());
    }
    kinds
}

/// Why this body cannot be admitted, or "" when it can. Oversized input is
/// refused BEFORE a send, never truncated into one.
pub fn body_refusal(body: &str, max_body_bytes: i64) -> String {
    let bytes = count_i64(body.len());
    if max_body_bytes <= 0 || bytes <= max_body_bytes {
        return String::new();
    }
    format!(
        "this message is {bytes} bytes; the network admits at most {max_body_bytes} — shorten it"
    )
}

/// Whether the send button may fire: a roster seat that sends, a recipient, a
/// body inside the bound, and no send already in flight.
pub fn send_ready(
    may_send: bool,
    sending: bool,
    recipient: &Option<String>,
    kind: &Option<String>,
    body: &str,
    max_body_bytes: i64,
) -> bool {
    let addressed = recipient.as_ref().is_some_and(|to| !to.is_empty());
    let kinded = kind.as_ref().is_some_and(|kind| !kind.is_empty());
    let written = !body.trim().is_empty();
    let within_bounds = body_refusal(body, max_body_bytes).is_empty();
    may_send && !sending && addressed && kinded && written && within_bounds
}

/// The participants this viewer may address: the roster minus itself. A send
/// names ONE recipient — group delivery is per-recipient messages.
pub fn recipients(roster: &[MessagingSeat]) -> Vec<String> {
    roster
        .iter()
        .filter(|seat| !seat.you)
        .map(|seat| seat.participant.clone())
        .collect()
}

/// The cursor an "older" page asks for: one page of EVENTS back, never below
/// the retained floor.
///
/// The step is the page size the app asked the network for, not the messages
/// on screen. A page is a page of the committed event stream — delivery and
/// binding events take sequences in it too — so stepping by the message count
/// would understep and redraw most of the page it just left.
pub fn older_from(from_seq: i64, floor_seq: i64, page_size: i64) -> i64 {
    (from_seq - page_size.max(1)).max(floor_seq)
}

/// The cursor a "newer" page asks for: one page on, and never past the
/// conversation's tip.
pub fn newer_from(from_seq: i64, page_size: i64, next_seq: i64) -> i64 {
    (from_seq + page_size.max(1)).min(next_seq).max(0)
}

pub fn reply_note(reply_to: i64) -> String {
    if reply_to <= 0 {
        return String::new();
    }
    format!("replying to sequence {reply_to}")
}

/// Whether the panel's reading still belongs to the scope on screen: the same
/// network, the same acting participant, the same conversation. The app fences
/// its own answers too — this is the half the guest can see, and it is what
/// keeps a draft written for A from being cleared by an answer about B.
pub fn same_scope(
    network: &str,
    participant: &str,
    conversation: &str,
    other_network: &str,
    other_participant: &str,
    other_conversation: &str,
) -> bool {
    network == other_network
        && participant == other_participant
        && conversation == other_conversation
}

// ---- the messaging intents -------------------------------------------------

/// Open a conversation as one of the viewer's own participants — the explicit
/// association the spec asks for. No discovery scan, no provider socket.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenConversation {
    pub participant: String,
    pub conversation: String,
}

/// One bounded page of the committed event stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageRequest {
    pub from_seq: i64,
    /// ask for the tail instead of `from_seq` — what "Newest" means when the
    /// conversation has moved since this page was drawn
    pub newest: bool,
}

/// One send, exactly as the composer holds it. The app resolves the credential,
/// the sequence and the deadline; none of them are the guest's to invent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendMessage {
    pub kind: String,
    pub recipient: String,
    pub body: String,
    pub reply_to: i64,
}

pub fn open_conversation(participant: &str, conversation: &str) -> bool {
    notify(
        "agents.messaging_open",
        &OpenConversation {
            participant: participant.trim().to_owned(),
            conversation: conversation.trim().to_owned(),
        },
    )
}

pub fn page_messages(from_seq: i64, newest: bool) -> bool {
    notify(
        "agents.messaging_page",
        &PageRequest {
            from_seq: from_seq.max(0),
            newest,
        },
    )
}

pub fn send_message(kind: &str, recipient: &str, body: &str, reply_to: i64) -> bool {
    notify(
        "agents.messaging_send",
        &SendMessage {
            kind: kind.to_owned(),
            recipient: recipient.to_owned(),
            // the body is sent EXACTLY as written: no trim, no truncation, no
            // normalization. What was refused for length is refused, not cut.
            body: body.to_owned(),
            reply_to: reply_to.max(0),
        },
    )
}
