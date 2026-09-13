//! The Agents register as a view on the kernel contract: who may act, which
//! executor they run on, the skills they carry and what their runs did,
//! rendered from a wasm component the desktop app loads from a file.
//!
//! The kernel pushes session facts only (`agents.props`: connected, dark,
//! the signing account, and the run another tab opened for the reader). The
//! register, the run tracker and one run's journal are read here through
//! the kernel's `rpc.query` / `rpc.view`, re-read on every `rpc.live` hit
//! for the `runs` and `identity` planes, and a pause or a save leaves as
//! `op.submit` — the runs message the kernel signs with the seated key. The
//! endpoint, the key and the password never cross: a guest that sees no key
//! cannot leak one.
pub mod host;
use ducktape_view_guest::{
    kit, slots,
    wire::{self, ButtonPreset, Length, Node},
};

fn action(key: impl Into<String>, label: &str, message: Option<Message>) -> Node {
    kit::button(
        key,
        label,
        message.map(slots::message),
        ButtonPreset::Secondary,
    )
}

fn field(key: &str, hint: &str, value: &str, message: fn(String) -> Message) -> Node {
    kit::input(
        key,
        hint,
        value,
        slots::handler(Box::new(move |value| Some(message(value)))),
        None,
    )
}

fn places(key: &str, links: &[host::RunLink]) -> Node {
    kit::row(
        key,
        links.iter().enumerate().map(|(index, link)| {
            let key = format!("{key}/{index}");
            if link.url.is_empty() {
                return kit::text(key, &link.label);
            }
            action(key, &link.label, Some(Message::OpenPlace(link.url.clone())))
        }),
    )
}

fn resize(key: &str, message: fn(f64, f64) -> Message) -> Node {
    Node::ResizeHandle {
        key: key.into(),
        on_press: None,
        on_release: None,
        on_drag: Some(slots::handler(Box::new(move |(x, y)| Some(message(x, y))))),
        cursor: Some(wire::mouse::Cursor::ResizingHorizontally),
        content: Box::new(kit::sized(
            kit::container(format!("{key}/edge"), kit::text(format!("{key}/grip"), "⋮")),
            Some(Length::Fixed(8.)),
            Some(Length::Fill),
        )),
    }
}

impl AgentsView {
    fn view(&self) -> Node {
        let mut header = vec![kit::heading("agents/title", "Agents")];
        if self.connected {
            header.push(kit::text(
                "agents/summary",
                match self.panel.as_str() {
                    "runs" => host::runs_summary(&self.runs),
                    _ => host::agents_summary(self.connected, &self.rows),
                },
            ));
            for (panel, label) in [("registry", "Registry"), ("runs", "Runs")] {
                header.push(action(
                    format!("agents/panel/{panel}"),
                    label,
                    (self.panel != panel).then(|| Message::ChoosePanel(panel.into())),
                ));
            }
            if !self.account.is_empty() {
                header.push(action("agents/new", "New agent", Some(Message::OpenNew)));
            }
        }
        let mut content = vec![
            kit::row("agents/header", header),
            kit::text("agents/about", host::pane_note(&self.panel)),
        ];
        if !self.host_error.is_empty() {
            content.push(kit::text("agents/error", &self.host_error));
        }
        if !self.connected {
            content.push(kit::heading("agents/disconnected", "Not connected"));
            content.push(kit::text(
                "agents/connect-help",
                "Choose a network to read its agent registry.",
            ));
        } else {
            let body = match self.panel.as_str() {
                "runs" => self.runs_panel(),
                _ => self.registry_panel(),
            };
            content.push(body);
        }
        let viewport = slots::handler::<(f32, f32), Message>(Box::new(|(w, h)| {
            Some(Message::ViewportChanged(w.into(), h.into()))
        }));
        Node::Sensor {
            key: "agents/viewport".into(),
            reset: None,
            on_show: Some(viewport),
            on_resize: Some(viewport),
            on_hide: None,
            anticipate: None,
            delay: None,
            child: Box::new(kit::sized(
                kit::column("agents/root", content),
                Some(Length::Fill),
                Some(Length::Fill),
            )),
        }
    }

    fn registry_panel(&self) -> Node {
        let mut rows = Vec::new();
        if self.rows.is_empty() && self.answered && !self.creating {
            rows.push(kit::text(
                "agents/empty",
                "No model agents configured — models appear here with their capability and skills.",
            ));
        }
        for agent in &self.rows {
            let key = format!("agents/record/{}", agent.id);
            let standing = match agent.status.as_str() {
                "active" => "ACTIVE",
                "paused" => "PAUSED",
                other => other,
            };
            let owner = if agent.owner_handle.is_empty() {
                "unowned"
            } else {
                &agent.owner_handle
            };
            let mut button = kit::button_child(
                &key,
                kit::column(
                    format!("{key}/summary"),
                    [
                        kit::heading(format!("{key}/name"), &agent.name),
                        kit::text(format!("{key}/capability"), &agent.capability),
                        kit::text(format!("{key}/skills"), agent.skills.len().to_string()),
                        kit::text(format!("{key}/owner"), owner),
                        kit::text(format!("{key}/standing"), standing),
                        kit::text(
                            format!("{key}/working"),
                            if agent.live { "Working" } else { "" },
                        ),
                    ],
                ),
                Some(slots::message(Message::OpenAgent(agent.id.clone()))),
                ButtonPreset::Subtle,
            );
            if let Node::Button { label, checked, .. } = &mut button {
                *label = Some(agent.name.clone());
                *checked = Some(self.selected == agent.id);
            }
            rows.push(button);
        }
        let mut panes = vec![kit::scroll(
            "agents/registry-scroll",
            kit::column("agents/registry", rows),
        )];
        let editor_open = !self.selected.is_empty() || self.creating;
        if editor_open {
            panes.push(resize("agents/editor-resize", Message::EditorResized));
            panes.push(kit::sized(
                kit::container(
                    "agents/editor",
                    kit::scroll("agents/editor-scroll", self.editor()),
                ),
                Some(Length::Fixed(self.editor_width as f32)),
                Some(Length::Fill),
            ));
        }
        kit::sized(
            kit::row("agents/registry-panes", panes),
            None,
            Some(Length::Fill),
        )
    }

    fn runs_panel(&self) -> Node {
        let mut rows = Vec::new();
        if self.runs.is_empty() {
            rows.push(kit::text(
                "agents/no-runs",
                "No runs yet — every dispatch of an agent lands here with its journal.",
            ));
        }
        for run in &self.runs {
            let key = format!("agents/run/{}", run.run_id);
            let mut button = kit::button_child(
                &key,
                kit::column(
                    format!("{key}/summary"),
                    [
                        kit::heading(format!("{key}/agent"), &run.agent_name),
                        kit::text(format!("{key}/id"), &run.run_id),
                        kit::text(format!("{key}/origin"), &run.origin),
                        kit::text(format!("{key}/state"), &run.state),
                        kit::text(format!("{key}/dispatched"), &run.dispatched),
                        kit::text(format!("{key}/settled"), &run.settled),
                        kit::text(format!("{key}/actions"), format!("{} actions", run.actions)),
                        kit::text(
                            format!("{key}/pr"),
                            if run.pr_number > 0 {
                                format!("PR #{}", run.pr_number)
                            } else {
                                String::new()
                            },
                        ),
                    ],
                ),
                Some(slots::message(Message::OpenRunRow(run.run_id.clone()))),
                ButtonPreset::Subtle,
            );
            if let Node::Button { label, checked, .. } = &mut button {
                *label = Some(run.run_id.clone());
                *checked = Some(self.open_run == run.dispatch_id);
            }
            rows.push(button);
        }
        let mut panes = vec![kit::scroll(
            "agents/runs-scroll",
            kit::column("agents/runs", rows),
        )];
        if !self.open_run.is_empty() {
            panes.push(resize("agents/journal-resize", Message::JournalResized));
            panes.push(kit::sized(
                kit::container(
                    "agents/journal",
                    kit::scroll("agents/journal-scroll", self.journal_panel()),
                ),
                Some(Length::Fixed(self.journal_width as f32)),
                Some(Length::Fill),
            ));
        }
        kit::sized(
            kit::row("agents/run-panes", panes),
            None,
            Some(Length::Fill),
        )
    }

    fn journal_panel(&self) -> Node {
        let mut items = vec![
            kit::row(
                "agents/journal-heading",
                [
                    kit::heading("agents/journal-run", &self.open_row.run_id),
                    kit::text("agents/journal-state", &self.open_row.state),
                    action(
                        "agents/close-journal",
                        "Close journal",
                        Some(Message::CloseRun),
                    ),
                ],
            ),
            action(
                "agents/receipt",
                "Run details",
                Some(Message::ToggleReceipt(self.open_run.clone())),
            ),
        ];
        if self.expanded_receipt == self.open_run {
            items.push(kit::text("agents/dispatch-id", &self.open_run));
            items.push(kit::text(
                "agents/output-reference",
                &self.open_row.output_ref,
            ));
        }
        if self.live.present {
            items.push(kit::heading("agents/live-state", &self.live.status));
            for (index, activity) in self.live.activity.iter().enumerate() {
                items.push(kit::row(
                    format!("agents/activity/{index}"),
                    [
                        kit::text(
                            format!("agents/activity/{index}/state"),
                            if activity.done { "✓" } else { "…" },
                        ),
                        kit::text(format!("agents/activity/{index}/label"), &activity.label),
                    ],
                ));
            }
            items.push(kit::text("agents/answer", &self.live.answer_preview));
        }
        let journal_ready = self.journal.dispatch_id == self.open_run;
        if journal_ready && !self.journal.links.is_empty() {
            items.push(kit::heading("agents/relevant", "Relevant"));
            items.push(places("agents/places", &self.journal.links));
        }
        if !self.open_row.reason.is_empty() {
            items.push(kit::text("agents/run-reason", &self.open_row.reason));
        }
        items.push(kit::heading("agents/journal-title", "Journal"));
        if !journal_ready {
            items.push(kit::text("agents/journal-loading", "Reading the journal…"));
        } else if self.journal.entries.is_empty() {
            items.push(kit::text("agents/journal-empty", "This run's journal has no entries yet — the fold may still be catching up to the chain."));
        } else {
            for (index, entry) in self.journal.entries.iter().enumerate() {
                let key = format!("agents/entry/{index}");
                items.push(kit::column(
                    &key,
                    [
                        kit::text(format!("{key}/height"), &entry.height),
                        kit::text(format!("{key}/kind"), &entry.kind),
                        kit::text(format!("{key}/summary"), &entry.summary),
                        kit::text(format!("{key}/status"), &entry.status),
                        places(&format!("{key}/targets"), &entry.targets),
                    ],
                ));
            }
        }
        kit::column("agents/journal-content", items)
    }

    fn editor(&self) -> Node {
        let mut items = vec![kit::row(
            "agents/editor-heading",
            [
                kit::heading(
                    "agents/editor-title",
                    if self.creating {
                        "New agent"
                    } else {
                        &self.draft_name
                    },
                ),
                action(
                    "agents/close-editor",
                    "Close editor",
                    Some(Message::CloseEditor),
                ),
            ],
        )];
        let read_only = !self.can_edit && !self.creating;
        if read_only {
            items.push(kit::text(
                "agents/read-only",
                "Only this agent's controller can change its record. You are reading it.",
            ));
        }
        if !self.creating {
            items.push(kit::heading("agents/standing", "Standing"));
            items.push(kit::text("agents/status", &self.selected_status));
            if self.can_edit {
                match self.selected_status.as_str() {
                    "active" => items.push(action(
                        "agents/pause",
                        "Pause agent",
                        Some(Message::SetStatus(self.selected.clone(), true)),
                    )),
                    "paused" => items.push(action(
                        "agents/resume",
                        "Resume agent",
                        Some(Message::SetStatus(self.selected.clone(), false)),
                    )),
                    _ => {}
                }
            }
        }
        items.push(kit::heading("agents/identity", "Identity"));
        if self.creating {
            items.push(field(
                "agents/id",
                "a-dns-label, e.g. chiefduck",
                &self.draft_id,
                Message::BindDraftId,
            ));
            let invalid_id = !self.draft_id.is_empty() && !host::valid_agent_id(&self.draft_id);
            if invalid_id {
                items.push(kit::text("agents/id-error", "An agent id is a lowercase DNS label: a-z, 0-9 and hyphens, no hyphen at either end."));
            }
        } else {
            items.push(kit::text("agents/id", &self.draft_id));
        }
        items.push(field(
            "agents/name",
            "display name…",
            &self.draft_name,
            Message::BindDraftName,
        ));
        items.push(kit::heading("agents/executor", "Executor"));
        let capability = host::or_empty(&self.draft_capability);
        if self.can_edit {
            let options = host::capability_options(&self.capabilities, &capability);
            let choices = options.clone();
            items.push(Node::PickList {
                key: "AgentsView/root/editor/agent-capability".into(),
                selected: options
                    .iter()
                    .position(|value| value == &capability)
                    .map(|index| index as u32),
                options,
                placeholder: Some("pick a capability…".into()),
                on_select: slots::handler(Box::new(move |index: u32| {
                    choices
                        .get(index as usize)
                        .cloned()
                        .map(Message::PickCapabilityOption)
                })),
                width: Some(Length::Fill),
                style: Default::default(),
                settings: Default::default(),
            });
        } else {
            items.push(kit::text("agents/capability", &capability));
        }
        items.push(kit::heading("agents/skills", "Skills"));
        for skill in &self.draft_skills {
            let key = format!("agents/skill/{}", skill.name);
            let mut details = vec![
                kit::heading(format!("{key}/name"), &skill.name),
                kit::text(format!("{key}/source"), &skill.source_prefix),
                kit::text(format!("{key}/snapshot"), &skill.source_snapshot),
            ];
            if self.can_edit {
                details.push(action(
                    format!("{key}/load"),
                    if skill.always {
                        "Load on demand"
                    } else {
                        "Load always"
                    },
                    Some(Message::LoadSkill(skill.name.clone(), !skill.always)),
                ));
                details.push(action(
                    format!("{key}/remove"),
                    "Remove skill",
                    Some(Message::RemoveSkill(skill.name.clone())),
                ));
            } else {
                details.push(kit::text(
                    format!("{key}/mode"),
                    host::skill_mode(skill.always),
                ));
            }
            items.push(kit::column(key, details));
        }
        if self.can_edit {
            items.extend([
                field(
                    "agents/skill-name",
                    "skill name (its mount directory)…",
                    &self.skill_name,
                    Message::BindSkillName,
                ),
                field(
                    "agents/skill-prefix",
                    "/shared/skills/<name> when left empty",
                    &self.skill_prefix,
                    Message::BindSkillPrefix,
                ),
                field(
                    "agents/skill-snapshot",
                    "snapshot id to pin (optional)",
                    &self.skill_snapshot,
                    Message::BindSkillSnapshot,
                ),
                Node::Toggle {
                    key: "agents/skill-always".into(),
                    kind: wire::ToggleKind::Checkbox,
                    label: "load always (persona)".into(),
                    checked: self.skill_always,
                    on_toggle: Some(slots::handler(Box::new(|value| {
                        Some(Message::SetSkillAlways(value))
                    }))),
                    style: Default::default(),
                    width: Some(Length::Fill),
                },
                action(
                    "agents/add-skill",
                    "Add skill",
                    (!self.skill_name.trim().is_empty()).then_some(Message::AddSkill),
                ),
            ]);
        }
        let complete_record = !self.draft_name.trim().is_empty() && !capability.is_empty();
        if self.creating {
            items.push(action(
                "agents/register",
                "Register agent",
                (complete_record && host::valid_agent_id(&self.draft_id))
                    .then_some(Message::SubmitRegister),
            ));
        } else if self.can_edit {
            items.push(action(
                "agents/save",
                "Save agent",
                complete_record.then_some(Message::SubmitSave),
            ));
        }
        kit::column("agents/editor-content", items)
    }
}
#[derive(serde::Serialize, serde::Deserialize)]
pub struct AgentsView {
    pub(crate) rows: Vec<crate::host::AgentRow>,
    pub(crate) runs: Vec<crate::host::RunRow>,
    pub(crate) journal: crate::host::RunJournal,
    pub(crate) live: crate::host::LiveRun,
    pub(crate) panel: String,
    pub(crate) open_run: String,
    pub(crate) journal_width: f64,
    pub(crate) editor_width: f64,
    pub(crate) viewport_width: f64,
    pub(crate) expanded_receipt: String,
    pub(crate) open_row: crate::host::RunRow,
    pub(crate) opened: i64,
    pub(crate) capabilities: Vec<String>,
    pub(crate) account: String,
    pub(crate) committed: i64,
    pub(crate) seeded: i64,
    pub(crate) connected: bool,
    pub(crate) connection_serial: i64,
    pub(crate) answered: bool,
    pub(crate) host_error: String,
    pub(crate) selected: String,
    pub(crate) creating: bool,
    pub(crate) can_edit: bool,
    pub(crate) selected_status: String,
    pub(crate) draft_id: String,
    pub(crate) draft_name: String,
    pub(crate) draft_capability: Option<String>,
    pub(crate) draft_skills: Vec<crate::host::AgentSkill>,
    pub(crate) skill_name: String,
    pub(crate) skill_prefix: String,
    pub(crate) skill_snapshot: String,
    pub(crate) skill_always: bool,
    pub(crate) sent: bool,
}
impl ::std::fmt::Debug for AgentsView {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("AgentsView")
    }
}
#[derive(Clone)]
pub enum Message {
    JournalResized(f64, f64),
    EditorResized(f64, f64),
    ViewportChanged(f64, f64),
    ToggleReceipt(String),
    SessionArrived(crate::host::SessionItem),
    RegisterArrived(crate::host::RegisterItem),
    JournalArrived(crate::host::JournalItem),
    LiveArrived(crate::host::LiveRun),
    ActDone(crate::host::ActItem),
    OpenAgent(String),
    OpenNew,
    CloseEditor,
    ChoosePanel(String),
    OpenRunRow(String),
    CloseRun,
    OpenPlace(String),
    PickCapabilityOption(String),
    SetSkillAlways(bool),
    AddSkill,
    RemoveSkill(String),
    LoadSkill(String, bool),
    SetStatus(String, bool),
    SubmitSave,
    SubmitRegister,
    BindDraftId(String),
    BindDraftName(String),
    BindSkillName(String),
    BindSkillPrefix(String),
    BindSkillSnapshot(String),
}
impl ::std::fmt::Debug for Message {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("Message")
    }
}
impl AgentsView {
    fn state() -> Self {
        Self {
            rows: Vec::new(),
            runs: Vec::new(),
            journal: crate::host::empty_journal(),
            live: crate::host::empty_live(),
            panel: "registry".to_owned(),
            open_run: "".to_owned(),
            journal_width: 400.0,
            editor_width: 400.0,
            viewport_width: 1280.0,
            expanded_receipt: "".to_owned(),
            open_row: crate::host::empty_run(),
            opened: 0,
            capabilities: Vec::new(),
            account: "".to_owned(),
            committed: 0,
            seeded: 0,
            connected: false,
            connection_serial: 0,
            answered: false,
            host_error: "".to_owned(),
            selected: "".to_owned(),
            creating: false,
            can_edit: false,
            selected_status: "".to_owned(),
            draft_id: "".to_owned(),
            draft_name: "".to_owned(),
            draft_capability: None,
            draft_skills: Vec::new(),
            skill_name: "".to_owned(),
            skill_prefix: "".to_owned(),
            skill_snapshot: "".to_owned(),
            skill_always: false,
            sent: false,
        }
    }
    pub(crate) fn boot() -> (Self, ::ducktape_view_guest::Task<Message>) {
        (Self::state(), ::ducktape_view_guest::Task::none())
    }
    pub(crate) const PREFERRED_WINDOW_SIZE: &'static str = "none";
    pub(crate) const SNAPSHOT_SCHEMA: &'static str =
        "ea29d77b6e03069ca0c580bd8bf52d687528c49195c03419d1d233b539e53355";
    pub(crate) fn snapshot(&self) -> Result<Vec<u8>, String> {
        self.validate_snapshot()?;
        wire::Snapshot {
            schema: Self::SNAPSHOT_SCHEMA.into(),
            state: wire::SnapshotValue::Bytes(wire::encode(self)),
        }
        .encode()
    }

    pub(crate) fn restore(bytes: &[u8]) -> Result<Self, String> {
        let snapshot = wire::Snapshot::decode(bytes)?;
        if snapshot.schema != Self::SNAPSHOT_SCHEMA {
            return Err("invalid Agents snapshot schema".into());
        }
        let wire::SnapshotValue::Bytes(state) = snapshot.state else {
            return Err("invalid Agents snapshot".into());
        };
        let state: Self = wire::decode(&state)?;
        state.validate_snapshot()?;
        Ok(state)
    }

    fn validate_snapshot(&self) -> Result<(), String> {
        let widths = [self.journal_width, self.editor_width, self.viewport_width];
        if widths.into_iter().all(f64::is_finite) {
            Ok(())
        } else {
            Err("invalid Agents snapshot geometry".into())
        }
    }
}
impl AgentsView {
    fn subscription(&self) -> ::ducktape_view_guest::Subscription<Message> {
        let run_open = self.connected && !self.open_run.is_empty();
        ::ducktape_view_guest::Subscription::batch([
            crate::host::session().map(Message::SessionArrived),
            if self.connected {
                host::register(self.connection_serial).map(Message::RegisterArrived)
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if run_open {
                host::run_journal(self.open_run.to_owned(), self.connection_serial)
                    .map(Message::JournalArrived)
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if run_open {
                host::live_run(self.open_run.to_owned(), self.connection_serial)
                    .map(Message::LiveArrived)
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            crate::host::acts().map(Message::ActDone),
        ])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_refuses_foreign_schema_corrupt_payload_and_nonfinite_geometry() {
        let (mut state, _) = AgentsView::boot();
        let mut envelope = wire::Snapshot::decode(&state.snapshot().unwrap()).unwrap();
        envelope.schema = "0".repeat(64);
        assert!(AgentsView::restore(&envelope.encode().unwrap()).is_err());
        envelope.schema = AgentsView::SNAPSHOT_SCHEMA.into();
        envelope.state = wire::SnapshotValue::Bytes(vec![255]);
        assert!(AgentsView::restore(&envelope.encode().unwrap()).is_err());
        state.journal_width = f64::INFINITY;
        assert!(state.snapshot().is_err());
        envelope.state = wire::SnapshotValue::Bytes(wire::encode(&state));
        assert!(AgentsView::restore(&envelope.encode().unwrap()).is_err());
    }

    #[test]
    fn disconnected_view_hides_retained_registry_editor_and_runs() {
        let (mut view, _) = AgentsView::boot();
        view.rows.push(host::AgentRow {
            id: "stale-agent".into(),
            ..Default::default()
        });
        view.selected = "stale-agent".into();
        view.creating = true;
        view.can_edit = true;
        view.account = "7".into();
        view.open_run = "stale-run".into();
        for pane in ["registry", "runs"] {
            view.panel = pane.into();
            let mut tree = view.view();
            tree.for_each_mut(&mut |node| {
                assert!(!node.key().is_some_and(|key| key.contains("stale-agent")
                    || key.ends_with("/editor")
                    || key.ends_with("/journal")));
                assert!(!matches!(
                    node,
                    Node::Button {
                        on_press: Some(_),
                        ..
                    }
                ));
            });
        }
        assert_eq!(view.rows.len(), 1);
    }
    #[test]
    fn view_fits_default_stack() {
        let (app, _) = AgentsView::boot();
        let _ = app.view();
    }

    #[test]
    fn snapshot_preserves_editor_and_journal_state() {
        let (mut app, _) = AgentsView::boot();
        app.selected = "reviewer".into();
        app.draft_name = "Reviewer 한글".into();
        app.draft_capability = Some("review".into());
        app.draft_skills = vec![host::AgentSkill {
            name: "review".into(),
            source_prefix: "/shared/skills/review".into(),
            source_snapshot: "pinned".into(),
            always: true,
        }];
        app.rows.push(host::AgentRow {
            id: "reviewer".into(),
            name: "Reviewer 한글".into(),
            capability: "review".into(),
            skills: app.draft_skills.clone(),
            ..Default::default()
        });
        app.open_run = "dispatch".into();
        app.expanded_receipt = "dispatch".into();
        app.journal_width = 480.;
        app.editor_width = 470.;
        let bytes = app.snapshot().unwrap();
        let restored = AgentsView::restore(&bytes).unwrap();
        assert_eq!(restored.snapshot().unwrap(), bytes);
    }
}
impl AgentsView {
    pub(crate) fn update(&mut self, message: Message) -> ::ducktape_view_guest::Task<Message> {
        match message {
            Message::JournalResized(dx, _dy) => self.on_journal_resized(dx, _dy),
            Message::EditorResized(dx, _dy) => self.on_editor_resized(dx, _dy),
            Message::ViewportChanged(width, _height) => self.on_viewport_changed(width, _height),
            Message::ToggleReceipt(value) => self.on_toggle_receipt(value),
            Message::SessionArrived(item) => self.on_session_arrived(item),
            Message::RegisterArrived(item) => self.on_register_arrived(item),
            Message::JournalArrived(item) => self.on_journal_arrived(item),
            Message::LiveArrived(item) => self.on_live_arrived(item),
            Message::ActDone(item) => self.on_act_done(item),
            Message::OpenAgent(id) => self.on_open_agent(id),
            Message::OpenNew => self.on_open_new(),
            Message::CloseEditor => self.on_close_editor(),
            Message::ChoosePanel(next) => self.on_choose_panel(next),
            Message::OpenRunRow(run_id) => self.on_open_run_row(run_id),
            Message::CloseRun => self.on_close_run(),
            Message::OpenPlace(url) => self.on_open_place(url),
            Message::PickCapabilityOption(value) => self.on_pick_capability_option(value),
            Message::SetSkillAlways(on) => self.on_set_skill_always(on),
            Message::AddSkill => self.on_add_skill(),
            Message::RemoveSkill(name) => self.on_remove_skill(name),
            Message::LoadSkill(name, always) => self.on_load_skill(name, always),
            Message::SetStatus(agent_id, paused) => self.on_set_status(agent_id, paused),
            Message::SubmitSave => self.on_submit_save(),
            Message::SubmitRegister => self.on_submit_register(),
            Message::BindDraftId(value) => self.on_bind_draft_id(value),
            Message::BindDraftName(value) => self.on_bind_draft_name(value),
            Message::BindSkillName(value) => self.on_bind_skill_name(value),
            Message::BindSkillPrefix(value) => self.on_bind_skill_prefix(value),
            Message::BindSkillSnapshot(value) => self.on_bind_skill_snapshot(value),
        }
    }
    fn on_journal_resized(&mut self, dx: f64, _dy: f64) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.journal_width = crate::host::journal_width_after_delta(
                    self.journal_width,
                    -dx,
                    self.viewport_width,
                );
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_editor_resized(&mut self, dx: f64, _dy: f64) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.editor_width = crate::host::editor_width_after_delta(
                    self.editor_width,
                    -dx,
                    self.viewport_width,
                );
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_viewport_changed(
        &mut self,
        width: f64,
        _height: f64,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.viewport_width = width;
            }
            {
                self.journal_width =
                    crate::host::journal_width_after_delta(self.journal_width, 0.0, width);
            }
            {
                self.editor_width =
                    crate::host::editor_width_after_delta(self.editor_width, 0.0, width);
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_toggle_receipt(&mut self, value: String) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.expanded_receipt = crate::host::pick_str(
                    self.expanded_receipt != value,
                    ::std::convert::AsRef::as_ref(&(value)),
                    ::std::convert::AsRef::as_ref(&("")),
                );
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_session_arrived(
        &mut self,
        item: crate::host::SessionItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.host_error = item.error.to_owned();
            }
            if !(item.error).is_empty() {
                return ::ducktape_view_guest::Task::none();
            }
            let next = item.next.clone();
            {
                self.connection_serial = crate::host::connection_serial_after(
                    self.connected,
                    next.connected,
                    self.connection_serial,
                );
            }
            {
                self.connected = next.connected;
            }
            {
                self.account = next.account.to_owned();
            }
            {
                self.open_run = next.open_run.to_owned();
            }
            {
                self.open_row = crate::host::run_at(
                    ::std::convert::AsRef::as_ref(&(self.runs)),
                    ::std::convert::AsRef::as_ref(&(self.open_run)),
                );
            }
            let door_pressed = (next.opened != self.opened) && (!(next.open_run).is_empty());
            {
                self.opened = next.opened;
            }
            {
                self.panel = crate::host::pick_str(
                    door_pressed,
                    ::std::convert::AsRef::as_ref(&("runs")),
                    ::std::convert::AsRef::as_ref(&(self.panel)),
                );
            }
            let row = crate::host::row_named(
                ::std::convert::AsRef::as_ref(&(self.rows)),
                ::std::convert::AsRef::as_ref(&(self.selected)),
            );
            {
                self.can_edit = crate::host::editable(
                    self.connected,
                    ::std::convert::AsRef::as_ref(&(self.account)),
                    ::std::convert::AsRef::as_ref(&(row.controller)),
                );
            }
            {}
            {}
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_register_arrived(
        &mut self,
        item: crate::host::RegisterItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.host_error = item.error.to_owned();
            }
            {
                self.answered = true;
            }
            if !(item.error).is_empty() {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.rows = item.rows.clone();
            }
            {
                self.runs = item.runs.clone();
            }
            {
                self.capabilities = item.capabilities.clone();
            }
            {
                self.open_row = crate::host::run_at(
                    ::std::convert::AsRef::as_ref(&(self.runs)),
                    ::std::convert::AsRef::as_ref(&(self.open_run)),
                );
            }
            {
                self.sent = crate::host::badge(crate::host::working_agents(
                    ::std::convert::AsRef::as_ref(&(self.rows)),
                ));
            }
            let consumed = crate::host::drafts_consumed(
                self.committed,
                self.seeded,
                self.creating,
                ::std::convert::AsRef::as_ref(&(self.rows)),
                ::std::convert::AsRef::as_ref(&(self.draft_id)),
            );
            {
                self.seeded = self.committed;
            }
            {
                self.selected = crate::host::pick_str(
                    consumed && self.creating,
                    ::std::convert::AsRef::as_ref(&(self.draft_id)),
                    ::std::convert::AsRef::as_ref(&(self.selected)),
                );
            }
            {
                self.creating = self.creating && (!consumed);
            }
            let row = crate::host::row_named(
                ::std::convert::AsRef::as_ref(&(self.rows)),
                ::std::convert::AsRef::as_ref(&(self.selected)),
            );
            {
                self.can_edit = crate::host::editable(
                    self.connected,
                    ::std::convert::AsRef::as_ref(&(self.account)),
                    ::std::convert::AsRef::as_ref(&(row.controller)),
                );
            }
            {
                self.selected_status = row.status.to_owned();
            }
            {
                self.draft_name = crate::host::pick_str(
                    consumed,
                    ::std::convert::AsRef::as_ref(&(row.name)),
                    ::std::convert::AsRef::as_ref(&(self.draft_name)),
                );
            }
            {
                self.draft_capability = crate::host::pick_capability(
                    consumed,
                    ::std::convert::AsRef::as_ref(&(row.capability)),
                    ::std::borrow::Borrow::borrow(&(self.draft_capability)),
                );
            }
            {
                self.draft_skills = crate::host::pick_skills(
                    consumed,
                    ::std::convert::AsRef::as_ref(&(row.skills)),
                    ::std::convert::AsRef::as_ref(&(self.draft_skills)),
                );
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_journal_arrived(
        &mut self,
        item: crate::host::JournalItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.host_error = item.error.to_owned();
            }
            if (!(item.error).is_empty()) || (item.journal.dispatch_id != self.open_run) {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.journal = item.journal.clone();
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_live_arrived(
        &mut self,
        item: crate::host::LiveRun,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.live = item.clone();
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_act_done(&mut self, item: crate::host::ActItem) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.host_error = item.error.to_owned();
            }
            if !(item.error).is_empty() {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.committed += 1;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_open_agent(&mut self, id: String) -> ::ducktape_view_guest::Task<Message> {
        {
            let row = crate::host::row_named(
                ::std::convert::AsRef::as_ref(&(self.rows)),
                ::std::convert::AsRef::as_ref(&(id)),
            );
            {
                self.selected = id.to_owned();
            }
            {
                self.creating = false;
            }
            {
                self.can_edit = crate::host::editable(
                    self.connected,
                    ::std::convert::AsRef::as_ref(&(self.account)),
                    ::std::convert::AsRef::as_ref(&(row.controller)),
                );
            }
            {
                self.selected_status = row.status.to_owned();
            }
            {
                self.draft_id = row.id.to_owned();
            }
            {
                self.draft_name = row.name.to_owned();
            }
            {
                self.draft_capability =
                    crate::host::some_str(::std::convert::AsRef::as_ref(&(row.capability)));
            }
            {
                self.draft_skills = row.skills.clone();
            }
            {
                self.skill_name = "".to_owned();
            }
            {
                self.skill_prefix = "".to_owned();
            }
            {
                self.skill_snapshot = "".to_owned();
            }
            {
                self.skill_always = false;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_open_new(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.panel = "registry".to_owned();
            }
            {
                self.selected = "".to_owned();
            }
            {
                self.creating = true;
            }
            {
                self.can_edit = self.connected && (!(self.account).is_empty());
            }
            {
                self.draft_id = "".to_owned();
            }
            {
                self.draft_name = "".to_owned();
            }
            {
                self.draft_capability = None;
            }
            {
                self.draft_skills = Vec::new();
            }
            {
                self.skill_name = "".to_owned();
            }
            {
                self.skill_prefix = "".to_owned();
            }
            {
                self.skill_snapshot = "".to_owned();
            }
            {
                self.skill_always = false;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_close_editor(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.selected = "".to_owned();
            }
            {
                self.creating = false;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_choose_panel(&mut self, next: String) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.panel = next.to_owned();
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_open_run_row(&mut self, run_id: String) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.expanded_receipt = "".to_owned();
            }
            {
                self.open_row = crate::host::run_named(
                    ::std::convert::AsRef::as_ref(&(self.runs)),
                    ::std::convert::AsRef::as_ref(&(run_id)),
                );
            }
            {
                self.open_run = self.open_row.dispatch_id.to_owned();
            }
            {
                self.sent = crate::host::open_run(::std::convert::AsRef::as_ref(
                    &(self.open_row.dispatch_id),
                ));
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_close_run(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.expanded_receipt = "".to_owned();
            }
            {
                self.open_run = "".to_owned();
            }
            {
                self.open_row = crate::host::empty_run();
            }
            {
                self.journal = crate::host::empty_journal();
            }
            {
                self.live = crate::host::empty_live();
            }
            {
                self.sent = crate::host::open_run(::std::convert::AsRef::as_ref(&("")));
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_open_place(&mut self, url: String) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::open_link(::std::convert::AsRef::as_ref(&(url)));
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_pick_capability_option(&mut self, value: String) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.draft_capability = Some(value.to_owned());
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_set_skill_always(&mut self, on: bool) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.skill_always = on;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_add_skill(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.draft_skills = crate::host::with_skill(
                    ::std::convert::AsRef::as_ref(&(self.draft_skills)),
                    ::std::convert::AsRef::as_ref(&(self.skill_name)),
                    ::std::convert::AsRef::as_ref(
                        &(crate::host::pick_str(
                            ((self.skill_prefix).trim().to_owned()).is_empty(),
                            ::std::convert::AsRef::as_ref(
                                &(crate::host::library_prefix(::std::convert::AsRef::as_ref(
                                    &(self.skill_name),
                                ))),
                            ),
                            ::std::convert::AsRef::as_ref(&(self.skill_prefix)),
                        )),
                    ),
                    ::std::convert::AsRef::as_ref(&(self.skill_snapshot)),
                    self.skill_always,
                );
            }
            {
                self.skill_name = "".to_owned();
            }
            {
                self.skill_prefix = "".to_owned();
            }
            {
                self.skill_snapshot = "".to_owned();
            }
            {
                self.skill_always = false;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_remove_skill(&mut self, name: String) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.draft_skills = crate::host::without_skill(
                    ::std::convert::AsRef::as_ref(&(self.draft_skills)),
                    ::std::convert::AsRef::as_ref(&(name)),
                );
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_load_skill(
        &mut self,
        name: String,
        always: bool,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.draft_skills = crate::host::skill_loaded(
                    ::std::convert::AsRef::as_ref(&(self.draft_skills)),
                    ::std::convert::AsRef::as_ref(&(name)),
                    always,
                );
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_set_status(
        &mut self,
        agent_id: String,
        paused: bool,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::status(::std::convert::AsRef::as_ref(&(agent_id)), paused);
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_submit_save(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::save(
                    ::std::convert::AsRef::as_ref(&(self.selected)),
                    ::std::convert::AsRef::as_ref(&(self.draft_name)),
                    ::std::convert::AsRef::as_ref(
                        &(crate::host::or_empty(::std::borrow::Borrow::borrow(
                            &(self.draft_capability),
                        ))),
                    ),
                    ::std::convert::AsRef::as_ref(&(self.draft_skills)),
                );
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_submit_register(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::register_agent(
                    ::std::convert::AsRef::as_ref(&(self.draft_id)),
                    ::std::convert::AsRef::as_ref(&(self.draft_name)),
                    ::std::convert::AsRef::as_ref(
                        &(crate::host::or_empty(::std::borrow::Borrow::borrow(
                            &(self.draft_capability),
                        ))),
                    ),
                    ::std::convert::AsRef::as_ref(&(self.draft_skills)),
                );
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_bind_draft_id(&mut self, value: String) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.draft_id = value;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_bind_draft_name(&mut self, value: String) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.draft_name = value;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_bind_skill_name(&mut self, value: String) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.skill_name = value;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_bind_skill_prefix(&mut self, value: String) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.skill_prefix = value;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_bind_skill_snapshot(&mut self, value: String) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.skill_snapshot = value;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
}
ducktape_view_guest::export_app!(
    AgentsView,
    "Agents",
    "The registry of who may act, on which executor, and what their runs did.",
    ["agents"]
);
