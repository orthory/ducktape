//! Rendering for the native Agents screen.

use std::ops::Deref;

use iced::widget::{
    Space, button, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::icons::{self, Icon};
use crate::theme::{
    self, BODY, BODY_LG, CAPTION, HEADING, LABEL, MONO, Palette, RADIUS_LG, RADIUS_MD, RADIUS_SM,
    SANS, SANS_SEMIBOLD, TITLE,
};
use crate::ui;

use super::run_log::{SemanticLogKind, semantic_log_rows};
use super::*;

const HEADER_HEIGHT: f32 = 56.0;
const ROSTER_WIDTH: f32 = 286.0;

#[derive(Debug, Clone, Copy)]
struct Colors {
    palette: Palette,
    accent: Color,
}

impl Deref for Colors {
    type Target = Palette;

    fn deref(&self) -> &Self::Target {
        &self.palette
    }
}

/// ducktape-ui tokens for this screen's `Colors`, preserving the runtime accent
/// on the focus ring (`theme::ui_for` alone falls back to `ACCENTS[0]`).
fn tokens(p: Colors) -> crate::ui::theme::Theme {
    theme::ui_for(&p.palette).with_accent(p.accent)
}

const ACTIONS: [(&str, &str); 7] = [
    ("chat.post", "Reply in the thread it was mentioned in"),
    (
        "chat.post_message",
        "Start messages in any channel, on its own initiative",
    ),
    ("tasks.create", "Create tasks"),
    ("tasks.update_status", "Update task status"),
    ("pages.comment", "Comment on pages"),
    ("pages.set_checked", "Check off page todos"),
    ("duckfs.write_text", "Write files"),
];

pub fn view(state: &State, mode: theme::Mode, accent: Color) -> Element<'_, Message> {
    let p = Colors {
        palette: *theme::palette(mode),
        accent,
    };
    let data = match &state.data {
        Resource::Ready(data) => Some(data),
        _ => None,
    };
    let header = header(state, data, p);
    let body: Element<'_, Message> = match &state.data {
        Resource::Loading => center_state("Loading agents...", "", None, p),
        Resource::Empty => center_state(
            "No agent data",
            "The workspace has not loaded yet.",
            None,
            p,
        ),
        Resource::Error(error) => center_state(
            "Agents unavailable",
            error,
            Some(Message::Load),
            p,
        ),
        Resource::Ready(data) => match state.tab {
            Tab::Agents => agents_tab(state, data, p),
            Tab::AutoReply => auto_reply_tab(state, data, p),
            Tab::Activity => activity_tab(state, data, p),
        },
    };
    column![header, body]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn header<'a>(state: &'a State, data: Option<&'a AgentData>, p: Colors) -> Element<'a, Message> {
    let agents = data.map_or(0, |data| data.agents.len());
    let watches = data.map_or(0, |data| data.watches.len());
    let runs = data.map_or(0, |data| data.pending_runs.len());
    container(
        row![
            icon_tile(Icon::Agent, 30.0, p),
            text("Agents").font(SANS_SEMIBOLD).size(HEADING).color(p.ink),
            Space::new().width(Length::Fill),
            container(
                row![
                    header_tab("Agents", agents, Tab::Agents, state.tab, p),
                    header_tab("Auto-reply", watches, Tab::AutoReply, state.tab, p),
                    header_tab("Activity", runs, Tab::Activity, state.tab, p)
                ]
                .spacing(4)
                .padding(3)
            )
            .style(move |_| rounded_surface(p.sidebar, p.border, RADIUS_MD)),
            primary_button(
                "+ Add agent",
                data.is_some().then_some(Message::StartAdding),
                p
            )
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .height(HEADER_HEIGHT)
    .padding([0, 22])
    .align_y(Alignment::Center)
    .style(move |_| bottom_rule(p.paper, p.border_soft))
    .into()
}

fn agents_tab<'a>(state: &'a State, data: &'a AgentData, p: Colors) -> Element<'a, Message> {
    row![roster(state, data, p), detail_pane(state, data, p)]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn roster<'a>(state: &'a State, data: &'a AgentData, p: Colors) -> Element<'a, Message> {
    let selected = selected_agent(state).map(|agent| agent.id.as_str());
    // The ROSTER header stays pinned above the scroll (contract keeps it fixed);
    // it scrolled away with the list before.
    let header = row![
        section_label("ROSTER", p),
        Space::new().width(Length::Fill),
        text(format!("{} total", data.agents.len()))
            .font(MONO)
            .size(CAPTION)
            .color(p.muted_2)
    ]
    .padding([14, 14])
    .align_y(Alignment::Center);
    let mut list = column![];
    if data.agents.is_empty() {
        list = list.push(empty_state(
            "No agents yet",
            "Add an agent to get started.",
            p,
        ));
    } else {
        for agent in &data.agents {
            list = list.push(roster_row(
                agent,
                !state.adding && selected == Some(agent.id.as_str()),
                p,
            ));
        }
    }
    container(column![header, scrollable(list)])
        .width(ROSTER_WIDTH)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(p.sidebar)),
            border: Border {
                color: p.border_soft,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn roster_row(agent: &AgentRecord, selected: bool, p: Colors) -> Element<'static, Message> {
    let active = agent.status == AgentStatus::Active;
    // A 3px accent bar flush to the left edge marks the selected row (contract
    // `boxShadow: inset 3px 0 0 accent`); the list owns the frame, so the row
    // itself is borderless — a per-row 4-side box doubled every divider.
    let accent_bar = container(Space::new())
        .width(3)
        .height(Length::Fill)
        .style(move |_| surface(if selected { p.accent } else { Color::TRANSPARENT }));
    let select = button(
        row![
            accent_bar,
            row![
                avatar(&agent.display_name, 36.0, p.filled, p.on_filled, p),
                column![
                    text(agent.display_name.clone())
                        .font(SANS_SEMIBOLD)
                        .size(BODY_LG)
                        .color(if selected { p.accent } else { p.ink }),
                    row![
                        status_dot(if active { p.green } else { p.amber }),
                        text(if active { "Active" } else { "Paused" })
                            .font(SANS)
                            .size(LABEL)
                            .color(p.muted_3),
                        text("·").size(LABEL).color(p.icon_idle),
                        text(capability_short(&agent.capability))
                            .font(MONO)
                            .size(LABEL)
                            .color(p.muted_2)
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center)
                ]
                .spacing(3)
                .width(Length::Fill)
            ]
            .spacing(12)
            .padding(Padding {
                top: 12.0,
                right: 14.0,
                bottom: 12.0,
                left: 11.0,
            })
            .align_y(Alignment::Center)
            .width(Length::Fill)
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(0)
    .style(move |_, status| button::Style {
        background: (selected || matches!(status, button::Status::Hovered))
            .then_some(Background::Color(if selected { p.sunken } else { p.hover })),
        text_color: p.ink,
        border: Border::default(),
        ..Default::default()
    })
    .on_press(Message::SelectAgent(agent.id.clone()));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(
        iced_agent_plugin::Role::ListItem,
        agent.display_name.clone(),
        select,
    );
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    select.into()
}

fn detail_pane<'a>(state: &'a State, data: &'a AgentData, p: Colors) -> Element<'a, Message> {
    let content: Element<'a, Message> = if state.adding {
        register_form(state, data, p)
    } else if let Some(agent) = selected_agent(state) {
        agent_detail(state, data, agent, p)
    } else if state.explicit_selection
        && state.selected_agent_id.is_some()
        && !data.agents.is_empty()
    {
        missing_agent(state.selected_agent_id.as_deref().unwrap_or_default(), p)
    } else {
        no_agents(p)
    };
    container(scrollable(
        container(content).width(Length::Fill).padding(22),
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn register_form<'a>(state: &'a State, data: &'a AgentData, p: Colors) -> Element<'a, Message> {
    let draft = &state.register;
    let t = tokens(p);
    let ready = draft.ready(data) && !state.busy;
    let mut form = column![
        section_label("REGISTER AGENT", p),
        card(
            column![
                row![
                    avatar(
                        if draft.display_name.is_empty() { "AI" } else { &draft.display_name },
                        40.0,
                        p.filled,
                        p.on_filled,
                        p,
                    ),
                    column![
                        text("Add an agent").font(SANS_SEMIBOLD).size(BODY_LG).color(p.ink),
                        text("Give it a name, pick what it runs on, and curate the documents it carries.")
                            .font(SANS)
                            .size(BODY)
                            .color(p.muted_2)
                    ]
                    .spacing(3)
                    .width(Length::Fill),
                    pill("AGENT", p.accent, p)
                ]
                .spacing(12)
                .align_y(Alignment::Start),
                labeled_input(
                    "AGENT DISPLAY NAME",
                    "Triage Agent…",
                    &draft.display_name,
                    Message::RegisterNameChanged,
                    p,
                ),
                runs_on_picker(
                    &draft.capability,
                    &data.capabilities,
                    data.capability_status,
                    Message::RegisterCapabilityChanged,
                    p,
                ),
                skill_editor(state, p),
                permission_grid(&draft.allowed_actions, false, p),
                section_label("RESOURCE CAPS", p),
                ui::checkbox::checkbox(
                    "Can search the global skill library",
                    draft.library_read,
                    &t,
                )
                .on_toggle(Message::RegisterLibraryChanged),
                row![
                    labeled_mono_input(
                        "FORGE READ REPOSITORIES",
                        "repo names",
                        &draft.forge_read,
                        Message::RegisterForgeReadChanged,
                        p,
                    ),
                    labeled_mono_input(
                        "FORGE PUSH REPOSITORIES",
                        "repo names",
                        &draft.forge_push,
                        Message::RegisterForgePushChanged,
                        p,
                    )
                ]
                .spacing(9),
                row![
                    labeled_mono_input(
                        "ADDITIONAL DUCKFS READ PREFIXES",
                        "/shared/data /projects/demo",
                        &draft.duckfs_read,
                        Message::RegisterDuckfsReadChanged,
                        p,
                    ),
                    labeled_mono_input(
                        "DUCKFS WRITE PREFIXES",
                        "/shared/agents/my-agent",
                        &draft.duckfs_write,
                        Message::RegisterDuckfsWriteChanged,
                        p,
                    )
                ]
                .spacing(9),
                row![
                    labeled_mono_input(
                        "ALLOWED TOOL IDS",
                        "tool or MCP ids",
                        &draft.tools,
                        Message::RegisterToolsChanged,
                        p,
                    ),
                    labeled_mono_input(
                        "SECRET REFERENCES",
                        "opaque vault references",
                        &draft.secrets,
                        Message::RegisterSecretsChanged,
                        p,
                    )
                ]
                .spacing(9),
                row![
                    labeled_mono_input(
                        "PAGE WRITE ACCESS",
                        "page ids, comma separated, or *",
                        &draft.pages_write,
                        Message::RegisterPagesChanged,
                        p,
                    ),
                    labeled_mono_input(
                        "CONCURRENT PEER CALLS",
                        "0",
                        &draft.subagent_budget,
                        Message::RegisterBudgetChanged,
                        p,
                    )
                ]
                .spacing(9),
                text("Maximum live calls across the recursive call tree; completed calls release their slot. 0 disables calls and the runtime hard cap is 8.")
                    .font(SANS)
                    .size(BODY)
                    .color(p.muted_2),
                secondary_button(
                    if draft.advanced { "Hide advanced" } else { "Advanced" },
                    Some(Message::ToggleRegisterAdvanced),
                    p,
                ),
                if draft.advanced {
                    labeled_input(
                        "AGENT ID",
                        "derived from display name",
                        &draft.id_override,
                        Message::RegisterIdChanged,
                        p,
                    )
                } else {
                    text(format!("Agent id: {}", draft.id())).font(MONO).size(LABEL).color(p.muted_2).into()
                },
                row![
                    Space::new().width(Length::Fill),
                    secondary_button("Cancel", Some(Message::CancelAdding), p),
                    primary_button(
                        if state.busy { "Registering..." } else { "Register agent" },
                        ready.then_some(Message::Register),
                        p,
                    )
                ]
                .spacing(8)
            ]
            .spacing(14)
            .padding(16),
            p,
        )
    ]
    .spacing(9);
    if let Some(error) = &state.error {
        form = form.push(error_banner(error, p));
    }
    form.into()
}

fn skill_editor(state: &State, p: Colors) -> Element<'_, Message> {
    let draft = &state.register;
    let t = tokens(p);
    let mut skills = column![section_label("CURATED SKILLS", p)].spacing(7);
    for (index, skill) in draft.skills.iter().enumerate() {
        skills = skills.push(
            row![
                pill(
                    if skill.load == LoadMode::Always {
                        "ALWAYS"
                    } else {
                        "ON DEMAND"
                    },
                    if skill.load == LoadMode::Always {
                        p.accent
                    } else {
                        p.purple
                    },
                    p
                ),
                text(skill.name.clone())
                    .font(SANS_SEMIBOLD)
                    .size(BODY)
                    .color(p.ink),
                text(skill.source_prefix.clone())
                    .font(MONO)
                    .size(LABEL)
                    .color(p.muted_2)
                    .width(Length::Fill)
                    .wrapping(text::Wrapping::WordOrGlyph),
                secondary_button("Remove", Some(Message::RemoveSkill(index)), p)
            ]
            .spacing(9)
            .align_y(Alignment::Center),
        );
    }
    skills = skills.push(
        row![
            sem_input(
                "Skill name",
                &draft.skill_name,
                ui::input::input("Skill name", &draft.skill_name, &t)
                    .on_input(Message::SkillNameChanged),
            ),
            sem_input(
                "Skill prefix",
                &draft.skill_prefix,
                ui::input::input("/skills/name", &draft.skill_prefix, &t)
                    .font(MONO)
                    .on_input(Message::SkillPrefixChanged),
            ),
            secondary_button(
                if draft.skill_load == LoadMode::Always {
                    "Always"
                } else {
                    "On demand"
                },
                Some(Message::SkillLoadChanged(
                    if draft.skill_load == LoadMode::Always {
                        LoadMode::OnDemand
                    } else {
                        LoadMode::Always
                    }
                )),
                p,
            ),
            secondary_button(
                "Add",
                (!draft.skill_name.trim().is_empty() && draft.skill_prefix.trim().starts_with('/'))
                    .then_some(Message::AddSkill),
                p,
            )
        ]
        .spacing(7),
    );
    skills.into()
}

fn agent_detail<'a>(
    state: &'a State,
    data: &'a AgentData,
    agent: &'a AgentRecord,
    p: Colors,
) -> Element<'a, Message> {
    let active = agent.status == AgentStatus::Active;
    let identity = container(
        row![
            avatar(&agent.display_name, 50.0, p.accent, Color::WHITE, p),
            column![
                text(agent.display_name.clone())
                    .font(SANS_SEMIBOLD)
                    .size(HEADING)
                    .color(p.on_filled),
                row![
                    selectable_mono(&agent.id, mix(p.filled, p.on_filled, 0.7), p),
                    on_dark_pill(
                        if active { "ACTIVE" } else { "PAUSED" },
                        if active { p.green } else { p.amber },
                        p
                    )
                ]
                .spacing(9)
                .align_y(Alignment::Center)
            ]
            .spacing(6)
            .width(Length::Fill),
            on_dark_button(
                if state.editing.is_some() {
                    "Close edit"
                } else {
                    "Edit"
                },
                (!agent.pending && !state.busy).then_some(if state.editing.is_some() {
                    Message::CloseEditing
                } else {
                    Message::StartEditing
                }),
                p,
            ),
            on_dark_button(
                if active {
                    "Pause agent"
                } else {
                    "Resume agent"
                },
                (!agent.pending && !state.busy).then_some(Message::ToggleAgentStatus),
                p,
            )
        ]
        .spacing(14)
        .align_y(Alignment::Start),
    )
    .padding(Padding {
        top: 18.0,
        right: 18.0,
        bottom: 17.0,
        left: 18.0,
    })
    .style(move |_| surface(p.filled));

    let mut body = column![
        section_label("RUNS ON", p),
        container(capability_strip(&agent.capability, p))
            .padding([12, 14])
            .style(move |_| rounded_surface(p.sunken, p.border, RADIUS_MD)),
        section_label("IDENTITY", p),
    ]
    .spacing(8)
    .padding(18);
    // A legacy/invalid id has no address it can be reached at — show none, never
    // a fabricated one.
    if let Some(address) = agent_address(&agent.id) {
        body = body.push(info_row("Address", &address, p));
    }
    body = body
        .push(info_row("Owner", &agent.owner.label(), p))
        .push(info_row("Skills", &skills_summary(&agent.skills), p))
        .push(info_row("Updated", &agent.updated_at, p))
        .push(section_label("SKILLS", p))
        .push(
            text(
                "Always-loaded documents are pasted into every run — they are this agent's \
                 persona. The others are listed by name and opened only when a job calls for one.",
            )
            .font(SANS)
            .size(BODY)
            .color(p.muted_2),
        );
    if agent.skills.is_empty() {
        body = body.push(
            text("No skills curated — this agent runs on the task instructions alone.")
                .font(SANS)
                .size(BODY)
                .color(p.muted_2),
        );
    } else {
        for skill in &agent.skills {
            body = body.push(skill_row(skill, p));
        }
    }
    body = body.push(section_label("PERMISSIONS", p));
    if agent.allowed_actions.is_empty() {
        body = body.push(
            text("Can't take any actions yet.")
                .font(SANS)
                .size(BODY)
                .color(p.muted_2),
        );
    } else {
        let mut permissions = row![].spacing(7);
        for action in &agent.allowed_actions {
            permissions = permissions.push(pill(action, p.accent, p));
        }
        body = body.push(permissions.wrap());
    }
    body = body
        .push(section_label("RESOURCE CAPS", p))
        .push(resource_caps_chips(&agent.caps, p));
    if let Some(edit) = &state.editing {
        body = body.push(edit_form(state, data, edit, p));
    }
    if let Some(error) = &state.error {
        body = body.push(error_banner(error, p));
    }
    container(column![identity, body])
        .width(Length::Fill)
        .style(move |_| card_style(p))
        .into()
}

fn resource_caps_chips(caps: &ResourceCaps, p: Colors) -> Element<'static, Message> {
    let mut grants = row![].spacing(7);
    for grant in resource_grant_labels(caps) {
        grants = grants.push(pill(&grant, p.muted_3, p));
    }
    grants.wrap().into()
}

pub(super) fn resource_grant_labels(caps: &ResourceCaps) -> Vec<String> {
    let mut grants = Vec::new();
    for (label, values) in [
        ("Forge read", &caps.forge_read),
        ("Forge push", &caps.forge_push),
        ("DuckFS read", &caps.duckfs_read),
        ("DuckFS write", &caps.duckfs_write),
        ("Tool", &caps.tools),
        ("Secret", &caps.secrets),
        ("Page write", &caps.pages_write),
    ] {
        for value in values {
            grants.push(format!("{label}: {value}"));
        }
    }
    grants.push(format!(
        "Concurrent peer calls: {}",
        caps.subagent_budget.unwrap_or(0)
    ));
    grants
}

fn edit_form<'a>(
    state: &'a State,
    data: &'a AgentData,
    edit: &'a EditDraft,
    p: Colors,
) -> Element<'a, Message> {
    let t = tokens(p);
    let valid = !edit.display_name.trim().is_empty()
        && data.capabilities.contains(&edit.capability)
        && !edit.allowed_actions.is_empty()
        && !state.busy;
    let form = column![
        horizontal_divider(p),
        section_label("EDIT AGENT", p),
        labeled_input(
            "DISPLAY NAME",
            "Name",
            &edit.display_name,
            Message::EditNameChanged,
            p
        ),
        runs_on_picker(
            &edit.capability,
            &data.capabilities,
            data.capability_status,
            Message::EditCapabilityChanged,
            p,
        ),
        permission_grid(&edit.allowed_actions, true, p),
        section_label("RESOURCE CAPS", p),
        ui::checkbox::checkbox(
            "Can search the global skill library",
            edit.library_read,
            &t,
        )
        .on_toggle(Message::EditLibraryChanged),
        row![
            labeled_mono_input(
                "FORGE READ REPOSITORIES",
                "repo names",
                &edit.forge_read,
                Message::EditForgeReadChanged,
                p,
            ),
            labeled_mono_input(
                "FORGE PUSH REPOSITORIES",
                "repo names",
                &edit.forge_push,
                Message::EditForgePushChanged,
                p,
            )
        ]
        .spacing(9),
        row![
            labeled_mono_input(
                "ADDITIONAL DUCKFS READ PREFIXES",
                "/shared/data /projects/demo",
                &edit.duckfs_read,
                Message::EditDuckfsReadChanged,
                p,
            ),
            labeled_mono_input(
                "DUCKFS WRITE PREFIXES",
                "/shared/agents/my-agent",
                &edit.duckfs_write,
                Message::EditDuckfsWriteChanged,
                p,
            )
        ]
        .spacing(9),
        row![
            labeled_mono_input(
                "ALLOWED TOOL IDS",
                "tool or MCP ids",
                &edit.tools,
                Message::EditToolsChanged,
                p,
            ),
            labeled_mono_input(
                "SECRET REFERENCES",
                "opaque vault references",
                &edit.secrets,
                Message::EditSecretsChanged,
                p,
            )
        ]
        .spacing(9),
        row![
            labeled_mono_input(
                "PAGE WRITE ACCESS",
                "page ids, comma separated, or *",
                &edit.pages_write,
                Message::EditPagesChanged,
                p,
            ),
            labeled_mono_input(
                "CONCURRENT PEER CALLS",
                "0",
                &edit.subagent_budget,
                Message::EditBudgetChanged,
                p,
            )
        ]
        .spacing(9),
        text("Maximum live calls across the recursive call tree; completed calls release their slot. 0 disables calls and the runtime hard cap is 8.")
            .font(SANS)
            .size(BODY)
            .color(p.muted_2),
        row![
            Space::new().width(Length::Fill),
            secondary_button("Cancel", Some(Message::CloseEditing), p),
            primary_button(
                if state.busy {
                    "Saving..."
                } else {
                    "Save changes"
                },
                valid.then_some(Message::SaveEdit),
                p,
            )
        ]
        .spacing(8)
    ]
    .spacing(10)
    .padding([14, 0]);
    form.into()
}

fn permission_grid<'a>(selected: &'a [String], editing: bool, p: Colors) -> Element<'a, Message> {
    let t = tokens(p);
    let mut grid = column![section_label("PERMISSIONS", p)].spacing(7);
    for (action, label) in ACTIONS {
        let action_owned = action.to_owned();
        let checked = selected.iter().any(|value| value == action);
        grid = grid.push(
            ui::checkbox::checkbox(label, checked, &t).on_toggle(move |_| {
                if editing {
                    Message::ToggleEditAction(action_owned.clone())
                } else {
                    Message::ToggleRegisterAction(action_owned.clone())
                }
            }),
        );
    }
    grid.into()
}

fn skills_summary(skills: &[SkillRef]) -> String {
    if skills.is_empty() {
        return "none".into();
    }
    let always = skills
        .iter()
        .filter(|skill| skill.load == LoadMode::Always)
        .count();
    format!("{always} always · {} on demand", skills.len() - always)
}

fn skill_row(skill: &SkillRef, p: Colors) -> Element<'static, Message> {
    container(
        row![
            pill(
                if skill.load == LoadMode::Always {
                    "ALWAYS"
                } else {
                    "ON DEMAND"
                },
                if skill.load == LoadMode::Always {
                    p.accent
                } else {
                    p.purple
                },
                p
            ),
            text(skill.name.clone())
                .font(SANS_SEMIBOLD)
                .size(BODY)
                .color(p.ink),
            text(format!(
                "{}/SKILL.md",
                skill.source_prefix.trim_end_matches('/')
            ))
            .font(MONO)
            .size(LABEL)
            .color(p.muted_2)
            .width(Length::Fill)
            .wrapping(text::Wrapping::WordOrGlyph),
            secondary_button(
                "Open",
                Some(Message::OpenSkillFiles(skill.source_prefix.clone())),
                p,
            )
        ]
        .spacing(9)
        .align_y(Alignment::Center),
    )
    .padding([8, 10])
    .style(move |_| rounded_surface(p.paper, p.border, RADIUS_SM))
    .into()
}

fn missing_agent(id: &str, p: Colors) -> Element<'static, Message> {
    card(
        column![
            icon_tile(Icon::Agent, 46.0, p),
            text("Agent not found").font(SANS_SEMIBOLD).size(TITLE).color(p.ink),
            text(format!("{id} isn’t in this workspace’s roster — it may have been removed since it was mentioned."))
                .font(SANS)
                .size(BODY)
                .color(p.muted_2),
            primary_button("Back to the roster", Some(Message::ClearExplicitSelection), p)
        ]
        .spacing(10)
        .padding([40, 24])
        .align_x(Alignment::Center),
        p,
    )
}

fn no_agents(p: Colors) -> Element<'static, Message> {
    card(
        column![
            icon_tile(Icon::Agent, 46.0, p),
            text("No agents yet")
                .font(SANS_SEMIBOLD)
                .size(TITLE)
                .color(p.ink),
            text("Add your first agent to start automating chats and tasks.")
                .font(SANS)
                .size(BODY)
                .color(p.muted_2),
            primary_button("+ Add agent", Some(Message::StartAdding), p)
        ]
        .spacing(10)
        .padding([40, 24])
        .align_x(Alignment::Center),
        p,
    )
}

fn auto_reply_tab<'a>(state: &'a State, data: &'a AgentData, p: Colors) -> Element<'a, Message> {
    let mut rows = column![
        section_label("AUTO-REPLY", p),
        text("Choose which channels agents watch and when they answer.")
            .font(SANS)
            .size(BODY)
            .color(p.muted_2)
    ]
    .spacing(7)
    .padding(22);
    let mut watches = column![];
    if data.watches.is_empty() {
        watches = watches.push(empty_state(
            "No watched channels",
            "Add one below to let agents reply automatically.",
            p,
        ));
    } else {
        for (index, watch) in data.watches.iter().enumerate() {
            if index > 0 {
                watches = watches.push(horizontal_divider(p));
            }
            watches = watches.push(watch_row(watch, data, p));
        }
    }
    rows = rows.push(card(watches, p));
    rows = rows.push(watch_form(state, data, p));
    if let Some(error) = &state.error {
        rows = rows.push(error_banner(error, p));
    }
    scrollable(rows)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn watch_row(watch: &Watch, data: &AgentData, p: Colors) -> Element<'static, Message> {
    let label = data
        .channels
        .iter()
        .find(|channel| channel.id == watch.channel_id)
        .map_or(watch.channel_id.clone(), |channel| channel.name.clone());
    let policy = policy_text(&watch.policy, data);
    container(
        row![
            container(icons::view(Icon::Chat, 15.0, p.muted_2))
                .width(31)
                .height(31)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(move |_| rounded_surface(p.sunken, p.border, RADIUS_SM)),
            column![
                text(format!("# {label}")).font(MONO).size(BODY).color(p.ink),
                text(policy).font(SANS).size(BODY).color(p.muted_2)
            ]
            .spacing(2)
            .width(Length::Fill),
            secondary_button(
                "Turn off",
                (!watch.pending).then_some(Message::RemoveWatch(watch.channel_id.clone())),
                p,
            )
        ]
        .spacing(11)
        .align_y(Alignment::Center),
    )
    .padding([12, 14])
    .into()
}

fn watch_form<'a>(state: &'a State, data: &'a AgentData, p: Colors) -> Element<'a, Message> {
    let policy_ready =
        state.watch.policy != WatchPolicyKind::Assigned || !state.watch.assigned_agent.is_empty();
    let mut form = column![
        section_label("ADD A CHANNEL", p),
        labeled_pick_list(
            "CHANNEL",
            data.channels
                .iter()
                .map(|channel| PickOption {
                    value: channel.id.clone(),
                    label: channel.name.clone(),
                })
                .collect(),
            Some(state.watch.channel_id.clone()),
            "Choose a channel",
            "No channels yet — create one in Chat first.",
            Message::WatchChannelChanged,
            p,
        ),
        row![
            segment_button(
                "When mentioned",
                WatchPolicyKind::Mention,
                state.watch.policy,
                p
            ),
            segment_button("Every message", WatchPolicyKind::All, state.watch.policy, p),
            segment_button(
                "Take turns",
                WatchPolicyKind::RoundRobin,
                state.watch.policy,
                p
            ),
            segment_button(
                "Only a chosen agent",
                WatchPolicyKind::Assigned,
                state.watch.policy,
                p
            )
        ]
        .spacing(6)
    ]
    .spacing(9)
    .padding(16);
    if state.watch.policy == WatchPolicyKind::Assigned {
        form = form.push(labeled_pick_list(
            "WHICH AGENT",
            data.agents
                .iter()
                .map(|agent| PickOption {
                    value: agent.id.clone(),
                    label: agent.display_name.clone(),
                })
                .collect(),
            Some(state.watch.assigned_agent.clone()),
            "Choose an agent",
            "No agents yet — register one first.",
            Message::WatchAssignedChanged,
            p,
        ));
    }
    form = form.push(row![
        Space::new().width(Length::Fill),
        primary_button(
            if state.busy {
                "Adding..."
            } else {
                "Add auto-reply"
            },
            (!state.busy && !state.watch.channel_id.is_empty() && policy_ready)
                .then_some(Message::AddWatch),
            p,
        )
    ]);
    card(form, p)
}

fn activity_tab<'a>(state: &'a State, data: &'a AgentData, p: Colors) -> Element<'a, Message> {
    let mut body = column![
        job_worker(data, p),
        usage_card(data.usage.as_ref(), p),
        row![
            filter_button("All", RunFilter::All, state.run_filter, p),
            filter_button("Requested by you", RunFilter::Mine, state.run_filter, p)
        ]
        .spacing(6)
    ]
    .spacing(12)
    .padding(22);
    let runs: Vec<&PendingRun> = data
        .pending_runs
        .iter()
        .filter(|run| state.run_filter == RunFilter::All || run.requested_by_me)
        .collect();
    if runs.is_empty() {
        body = body.push(card(
            empty_state("No active runs", "In-flight agent work appears here.", p),
            p,
        ));
    } else {
        body = body.push(section_label("IN PROGRESS", p));
        for run in runs {
            body = body.push(run_row(run, data, state, p));
        }
    }
    body = body.push(
        row![
            section_label("HISTORY", p),
            text(data.recent_runs.len())
                .font(MONO)
                .size(CAPTION)
                .color(p.muted_2)
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    );
    if let Some(error) = &data.recent_runs_error {
        body = body.push(
            container(
                column![
                    text("Run history unavailable")
                        .font(SANS)
                        .size(LABEL)
                        .color(p.red),
                    selectable_line(error, p.red, p),
                ]
                .spacing(2),
            )
            .width(Length::Fill)
            .padding([7, 9])
            .style(move |_| {
                rounded_surface(
                    mix(p.paper, p.red, 0.09),
                    mix(p.paper, p.red, 0.25),
                    RADIUS_SM,
                )
            }),
        );
    }
    if data.recent_runs.is_empty() && data.recent_runs_error.is_none() {
        body = body.push(card(
            empty_state(
                "No delivered runs yet",
                "Finished runs land here; the node keeps the most recent 100.",
                p,
            ),
            p,
        ));
    } else {
        for run in &data.recent_runs {
            body = body.push(history_row(run, data, state, p));
        }
    }
    if let Some(error) = &state.error {
        body = body.push(error_banner(error, p));
    }
    scrollable(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn job_worker(data: &AgentData, p: Colors) -> Element<'static, Message> {
    let pending = data.job_worker_pending;
    card(
        row![
            icon_tile(Icon::Agent, 34.0, p),
            column![
                text("Jobs worker")
                    .font(SANS_SEMIBOLD)
                    .size(BODY_LG)
                    .color(p.ink),
                text("Let active agents claim work from the jobs board.")
                    .font(SANS)
                    .size(BODY)
                    .color(p.muted_2),
                text(if pending {
                    "Waiting for confirmation…"
                } else {
                    "Current committed status is not readable on this network."
                })
                .font(SANS)
                .size(BODY)
                .color(if pending { p.amber } else { p.muted_2 })
            ]
            .spacing(3)
            .width(Length::Fill),
            secondary_button(
                "Enable worker",
                (!pending).then_some(Message::SetJobWorker(true)),
                p,
            ),
            secondary_button(
                "Disable worker",
                (!pending).then_some(Message::SetJobWorker(false)),
                p,
            )
        ]
        .spacing(11)
        .padding([12, 14])
        .align_y(Alignment::Center),
        p,
    )
}

fn usage_card(usage: Option<&Usage>, p: Colors) -> Element<'static, Message> {
    let Some(usage) = usage else {
        return card(
            row![
                text("Usage").font(SANS_SEMIBOLD).size(BODY_LG).color(p.ink),
                Space::new().width(Length::Fill),
                text("No usage yet").font(SANS).size(BODY).color(p.muted_2)
            ]
            .padding([12, 14])
            .align_y(Alignment::Center),
            p,
        );
    };
    card(
        row![
            stat("REQUESTS", usage.requests.to_string(), p),
            stat("INPUT TOKENS", usage.input_tokens.to_string(), p),
            stat("OUTPUT TOKENS", usage.output_tokens.to_string(), p),
            stat("FAILED", usage.failed.to_string(), p),
            stat("BLOCKS", usage.duration_blocks.to_string(), p)
        ]
        .spacing(12)
        .padding(14),
        p,
    )
}

fn run_row(
    run: &PendingRun,
    data: &AgentData,
    state: &State,
    p: Colors,
) -> Element<'static, Message> {
    let agent = data
        .agents
        .iter()
        .find(|agent| agent.id == run.agent_id)
        .map_or(run.agent_id.clone(), |agent| agent.display_name.clone());
    let channel = data
        .channels
        .iter()
        .find(|channel| channel.id == run.channel_id)
        .map_or(run.channel_id.clone(), |channel| channel.name.clone());
    let expanded = state.expanded_run_logs.contains(&run.dispatch_id);
    // The lease is expired when no views remain — the run has stalled and can be
    // force-reassigned. `PendingRun` carries no `reassignable`/`maxAttempts`
    // flag yet (data-plane deferred), so an expired lease is the honest proxy:
    // it never offers reassign on a healthy run.
    // ponytail: expired-lease heuristic; swap for the node's `reassignable`
    // flag when the lease wire carries it.
    let reassignable = run.lease_remaining == Some(0);
    let (status_label, status_tone) = if reassignable {
        ("LEASE EXPIRED", p.red)
    } else {
        ("WORKING…", p.amber)
    };
    let identity = row![
        text(agent.clone())
            .font(SANS_SEMIBOLD)
            .size(BODY_LG)
            .color(p.ink),
        pill(status_label, status_tone, p),
        if run.job_id.is_some() {
            pill("JOB", p.purple, p)
        } else {
            pill("CHAT", p.blue, p)
        },
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let mut chips = row![text(if let Some(job) = &run.job_id {
        format!("job {job} · dispatch {}", short(&run.dispatch_id))
    } else {
        format!(
            "#{channel} · message {} · dispatch {}",
            run.anchor_sequence,
            short(&run.dispatch_id)
        )
    })
    .font(MONO)
    .size(LABEL)
    .color(p.muted_2)]
    .spacing(7)
    .align_y(Alignment::Center);
    if run.requested_by_me {
        chips = chips.push(pill("you", p.muted_3, p));
    }
    let mut actions = row![secondary_button(
        if expanded { "Hide log" } else { "Live log" },
        Some(Message::ToggleRunLog(run.dispatch_id.clone())),
        p,
    )]
    .spacing(8)
    .align_y(Alignment::Center);
    if reassignable {
        // Pass the current attempt (the node increments); an unconditional
        // `attempt + 1` double-incremented and the reassign was rejected.
        actions = actions.push(secondary_button(
            "Force reassign",
            (!run.pending).then_some(Message::ReassignRun(run.run_id.clone(), run.attempt)),
            p,
        ));
    }
    actions = actions.push(secondary_button(
        "Cancel",
        (!run.pending).then_some(Message::CancelRun(run.run_id.clone())),
        p,
    ));
    let mut content = column![
        column![
            row![
                avatar(&agent, 34.0, p.filled, p.on_filled, p),
                column![
                    identity.wrap(),
                    chips.wrap(),
                    text(run.created_at.clone())
                        .font(SANS)
                        .size(LABEL)
                        .color(p.muted_2)
                ]
                .spacing(4)
                .width(Length::Fill),
            ]
            .spacing(11)
            .align_y(Alignment::Center),
            actions.wrap(),
        ]
        .spacing(8)
        .padding([12, 14])
    ];
    if expanded {
        content = content.push(run_log_pane(&run.dispatch_id, false, state, p));
    }
    card(content, p)
}

fn history_row(
    run: &RunRecord,
    data: &AgentData,
    state: &State,
    p: Colors,
) -> Element<'static, Message> {
    let agent = data
        .agents
        .iter()
        .find(|agent| agent.id == run.agent_id)
        .map_or(run.agent_id.clone(), |agent| agent.display_name.clone());
    let channel = data
        .channels
        .iter()
        .find(|channel| channel.id == run.channel_id)
        .map_or(run.channel_id.clone(), |channel| channel.name.clone());
    let target = if run.channel_id.is_empty() {
        "job".into()
    } else {
        format!("#{channel} · message {}", run.anchor_sequence)
    };
    let expanded = state.expanded_run_logs.contains(&run.dispatch_id);
    let tone = if run.outcome == RunOutcome::Delivered {
        p.green
    } else {
        p.red
    };
    let mut metadata = row![
        pill(
            if run.outcome == RunOutcome::Delivered {
                "DELIVERED"
            } else {
                "FAILED"
            },
            tone,
            p,
        ),
        pill(&run_duration(run), p.muted_3, p),
    ]
    .spacing(7)
    .align_y(Alignment::Center);
    if !run.channel_id.is_empty() && run.anchor_sequence > 0 {
        metadata = metadata.push(run_link_button(
            target,
            Message::OpenRunAnchor {
                channel_id: run.channel_id.clone(),
                sequence: run.anchor_sequence,
            },
            p.blue,
            p,
        ));
    } else {
        metadata = metadata.push(text(target).font(MONO).size(LABEL).color(p.muted_2));
    }
    if run.degraded {
        metadata = metadata.push(pill("DEGRADED", p.amber, p));
    }
    if run.executing_node != "unknown" {
        metadata = metadata.push(pill(
            &format!("on {}", short(&run.executing_node)),
            p.purple,
            p,
        ));
    }
    if let Some(number) = run.pr_number {
        metadata = if forge_item_channel(&run.channel_id).is_some() {
            metadata.push(run_link_button(
                format!("PR #{number}"),
                Message::OpenRunPullRequest {
                    channel_id: run.channel_id.clone(),
                    number,
                },
                p.green,
                p,
            ))
        } else {
            metadata.push(pill(&format!("PR #{number}"), p.green, p))
        };
    }
    metadata = metadata.push(secondary_button(
        if expanded { "Hide log" } else { "Log" },
        Some(Message::ToggleRunLog(run.dispatch_id.clone())),
        p,
    ));
    if let Some(reference) = &run.output_ref {
        metadata = metadata.push(text(short(reference)).font(MONO).size(CAPTION).color(p.muted_2));
    }
    let mut content = column![
        row![
            avatar(&agent, 30.0, p.filled, p.on_filled, p),
            text(agent).font(SANS_SEMIBOLD).size(BODY_LG).color(p.ink)
        ]
        .spacing(9)
        .align_y(Alignment::Center),
        // A non-wrapping row of up to ~8 pills/buttons overflows a narrow pane and
        // clips the trailing Log button; `.wrap()` reflows to multiple lines.
        metadata.wrap(),
    ]
    .spacing(8)
    .padding([10, 12]);
    if expanded {
        content = content.push(run_log_pane(&run.dispatch_id, true, state, p));
    }
    card(content, p)
}

fn run_log_pane(
    dispatch_id: &str,
    terminal: bool,
    state: &State,
    p: Colors,
) -> Element<'static, Message> {
    let log = state.run_logs.get(dispatch_id);
    let mut output = column![];
    if let Some(dropped) = log.map(|log| log.dropped).filter(|dropped| *dropped > 0) {
        output = output.push(
            text(format!("live log tail: {dropped} older events omitted"))
                .font(MONO)
                .size(CAPTION)
                .color(p.amber),
        );
    }
    if let Some(log) = log {
        for row in semantic_log_rows(&log.entries) {
            if row.kind == SemanticLogKind::Blank {
                output = output.push(Space::new().height(4));
                continue;
            }
            let label = match row.kind {
                SemanticLogKind::Message => "message",
                SemanticLogKind::Command => "command",
                SemanticLogKind::Output => "output",
                SemanticLogKind::Status => "status",
                SemanticLogKind::Exit => "exit",
                SemanticLogKind::File => "files",
                SemanticLogKind::Tool => "tool",
                SemanticLogKind::Text => match row.stream {
                    Some(RunStream::Stderr) => "stderr",
                    _ => "stdout",
                },
                SemanticLogKind::Gap => "gap",
                SemanticLogKind::Blank => "",
            };
            let color = match row.kind {
                SemanticLogKind::Command | SemanticLogKind::Tool => p.blue,
                SemanticLogKind::Message | SemanticLogKind::File => p.ink,
                SemanticLogKind::Status => p.muted_2,
                SemanticLogKind::Exit if row.text == "exit: 0" => p.green,
                SemanticLogKind::Gap => p.amber,
                _ if row.stream == Some(RunStream::Stderr) => p.red,
                _ => p.ink_soft,
            };
            output = output.push(
                row![
                    text(label).font(MONO).size(CAPTION).color(p.muted_2).width(52),
                    text(row.text)
                        .font(MONO)
                        .size(LABEL)
                        .color(color)
                        .width(Length::Fill)
                        .wrapping(text::Wrapping::WordOrGlyph),
                ]
                .spacing(8),
            );
        }
    }
    let has_output = log.is_some_and(|log| !log.entries.is_empty());
    if !has_output {
        let unavailable = log.is_some_and(|log| log.unavailable);
        output = output.push(
            text(if unavailable {
                "Run output unavailable."
            } else if terminal {
                "No retained output received; older output may have been evicted."
            } else {
                "Waiting for retained output..."
            })
            .font(SANS)
            .size(BODY)
            .color(p.muted_2),
        );
    }
    // A read-only `text` has no selection, so the Copy button is the only way to
    // lift a command, stack trace, or error line out of the log — the pane's
    // whole reason to exist. It flattens the visible rows to the clipboard.
    let header = row![
        Space::new().width(Length::Fill),
        secondary_button(
            "Copy",
            has_output.then(|| Message::CopyRunLog(dispatch_id.to_owned())),
            p,
        )
    ]
    .align_y(Alignment::Center);
    // Shrink-to-content capped at 220 so a 3-line log is 3 lines tall (a dead
    // fixed-180 pane reserved blank space a short log never fills).
    let body = container(scrollable(output.spacing(3)).width(Length::Fill))
        .max_height(220.0)
        .padding([8, 10])
        .style(move |_| rounded_surface(p.canvas, p.border_soft, RADIUS_SM));
    column![header, body].spacing(6).into()
}

pub(super) fn run_duration(run: &RunRecord) -> String {
    const WALL_CLOCK_SECONDS_FLOOR: u64 = 978_307_200;
    const WALL_CLOCK_MILLIS_FLOOR: u64 = 978_307_200_000;
    let wall_seconds = |stamp: u64| {
        if stamp > WALL_CLOCK_MILLIS_FLOOR {
            Some(stamp / 1_000)
        } else if stamp > WALL_CLOCK_SECONDS_FLOOR {
            Some(stamp)
        } else {
            None
        }
    };
    if let (Some(start), Some(end)) = (wall_seconds(run.created_at), wall_seconds(run.delivered_at))
    {
        let seconds = end.saturating_sub(start);
        return match seconds {
            0 => "<1s".into(),
            1..=59 => format!("{seconds}s"),
            60..=3_599 => format!("{}m {}s", seconds / 60, seconds % 60),
            _ => format!("{}h {}m", seconds / 3_600, seconds % 3_600 / 60),
        };
    }
    let blocks = run.delivered_at.saturating_sub(run.created_at);
    if blocks == 1 {
        "1 block".into()
    } else {
        format!("{blocks} blocks")
    }
}

fn header_tab(
    label: &'static str,
    count: usize,
    tab: Tab,
    active: Tab,
    p: Colors,
) -> Element<'static, Message> {
    let btn = button(
        row![
            text(label).font(SANS_SEMIBOLD).size(LABEL),
            container(text(count).font(MONO).size(CAPTION).color(if tab == active {
                p.accent
            } else {
                p.muted_2
            }))
            .padding([0, 5])
            .style(move |_| rounded_surface(
                if tab == active {
                    mix(p.paper, p.accent, 0.09)
                } else {
                    p.sidebar
                },
                Color::TRANSPARENT,
                999.0
            ))
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .style(move |_, _| button::Style {
        background: (tab == active).then_some(Background::Color(p.paper)),
        text_color: if tab == active { p.accent } else { p.muted_2 },
        border: Border {
            radius: RADIUS_SM.into(),
            ..Default::default()
        },
        shadow: if tab == active {
            Shadow {
                color: Color { a: 0.07, ..p.shadow },
                offset: Vector::new(0.0, 1.0),
                blur_radius: 3.0,
            }
        } else {
            Shadow::default()
        },
        ..Default::default()
    })
    .on_press(Message::SelectTab(tab));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Tab, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn segment_button(
    label: &'static str,
    policy: WatchPolicyKind,
    active: WatchPolicyKind,
    p: Colors,
) -> Element<'static, Message> {
    let btn = button(text(label).font(SANS_SEMIBOLD).size(LABEL))
        .padding([6, 9])
        .style(move |_, _| button::Style {
            background: Some(Background::Color(if policy == active {
                mix(p.paper, p.accent, 0.09)
            } else {
                p.paper
            })),
            text_color: if policy == active {
                p.accent
            } else {
                p.muted_3
            },
            border: Border {
                color: if policy == active {
                    mix(p.paper, p.accent, 0.25)
                } else {
                    p.border
                },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        })
        .on_press(Message::WatchPolicyChanged(policy));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn filter_button(
    label: &'static str,
    filter: RunFilter,
    active: RunFilter,
    p: Colors,
) -> Element<'static, Message> {
    let btn = button(text(label).font(SANS_SEMIBOLD).size(LABEL))
        .padding([5, 10])
        .style(move |_, _| button::Style {
            background: Some(Background::Color(if filter == active {
                p.filled
            } else {
                p.paper
            })),
            text_color: if filter == active {
                p.on_filled
            } else {
                p.muted_2
            },
            border: Border {
                color: p.border_strong,
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        })
        .on_press(Message::SelectRunFilter(filter));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn capability_strip(capability: &str, p: Colors) -> Element<'static, Message> {
    let parts: Vec<&str> = capability.split('_').collect();
    let provider = title_case(parts.first().copied().unwrap_or(capability));
    let mut strip = row![
        text(provider)
            .font(SANS_SEMIBOLD)
            .size(BODY)
            .color(p.accent)
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if let Some(model) = parts.get(1) {
        strip = strip.push(text("›").font(MONO).size(LABEL).color(p.icon_idle));
        strip = strip.push(text((*model).to_owned()).font(MONO).size(BODY).color(p.ink));
    }
    if let Some(effort) = parts.get(2) {
        strip = strip.push(pill(&effort.to_uppercase(), p.accent, p));
    }
    strip.into()
}

fn capability_short(capability: &str) -> String {
    let mut parts = capability.split('_');
    let provider = title_case(parts.next().unwrap_or(capability));
    parts
        .next()
        .map_or(provider.clone(), |model| format!("{provider} · {model}"))
}

fn policy_text(policy: &TurnPolicy, data: &AgentData) -> String {
    match policy {
        TurnPolicy::Mention => "When mentioned".into(),
        TurnPolicy::All => "Every message".into(),
        TurnPolicy::RoundRobin => "Take turns".into(),
        TurnPolicy::Assigned(id) => format!(
            "Only {}",
            data.agents
                .iter()
                .find(|agent| &agent.id == id)
                .map_or(id.as_str(), |agent| agent.display_name.as_str())
        ),
    }
}

/// Dev-only text-input tagging: wraps `input` in a `TextInput` semantic node
/// carrying `value`. Compiled out entirely unless the agent bridge is built.
#[cfg(all(feature = "agent", debug_assertions))]
fn sem_input<'a>(
    name: &'static str,
    value: &str,
    input: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    iced_agent_plugin::Sem::new(iced_agent_plugin::Role::TextInput, name, input)
        .value(value.to_string())
        .into()
}
#[cfg(not(all(feature = "agent", debug_assertions)))]
fn sem_input<'a>(
    _name: &'static str,
    _value: &str,
    input: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    input.into()
}

/// A pick_list option that shows `label` but carries an opaque `value`.
#[derive(Clone, PartialEq, Eq)]
struct PickOption {
    value: String,
    label: String,
}

impl std::fmt::Display for PickOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// A labeled pick_list over records, or a disabled note when the list is empty.
/// Replaces the free-text channel/agent inputs (§B): typing a channel *name*
/// stored a bogus id and the watch never fired.
fn labeled_pick_list<'a>(
    label: &'static str,
    options: Vec<PickOption>,
    selected: Option<String>,
    placeholder: &'static str,
    empty_note: &'static str,
    on_select: fn(String) -> Message,
    p: Colors,
) -> Element<'a, Message> {
    let control: Element<'a, Message> = if options.is_empty() {
        disabled_note(empty_note, p)
    } else {
        let current = selected
            .as_ref()
            .and_then(|value| options.iter().find(|option| &option.value == value).cloned());
        pick_list(options, current, move |option: PickOption| on_select(option.value))
            .placeholder(placeholder)
            .text_size(BODY)
            .width(Length::Fill)
            .into()
    };
    column![section_label(label, p), control]
        .spacing(5)
        .width(Length::Fill)
        .into()
}

fn disabled_note(message: &'static str, p: Colors) -> Element<'static, Message> {
    container(text(message).font(SANS).size(BODY).color(p.muted_2))
        .width(Length::Fill)
        .padding([8, 10])
        .style(move |_| rounded_surface(p.sunken, p.border, RADIUS_SM))
        .into()
}

/// Parse a capability tag into `(provider, model, effort)`. A 3-part
/// `provider_model_effort` splits; a bare tag has no model/effort; any other
/// `_`-containing shape is opaque (the whole tag is its own provider key).
fn parse_capability_tag(tag: &str) -> (String, Option<String>, Option<String>) {
    if !tag.contains('_') {
        return (tag.to_owned(), None, None);
    }
    let parts: Vec<&str> = tag.split('_').collect();
    if parts.len() == 3 && parts.iter().all(|part| !part.is_empty()) {
        (parts[0].to_owned(), Some(parts[1].to_owned()), Some(parts[2].to_owned()))
    } else {
        (tag.to_owned(), None, None)
    }
}

#[derive(Clone)]
struct TagEntry {
    key: String,
    model: Option<String>,
    effort: Option<String>,
    tag: String,
    announced: bool,
}

/// The cascading provider → model → effort picker over `data.capabilities`
/// (§A). A free-text "RUNS ON" silently greyed Register/Save on any tag that
/// wasn't an exact announced string; here every pick resolves to one announced
/// (or pinned-offline) tag.
fn runs_on_picker<'a>(
    value: &'a str,
    capabilities: &'a [String],
    status: CapabilityStatus,
    on_change: fn(String) -> Message,
    p: Colors,
) -> Element<'a, Message> {
    if status != CapabilityStatus::Ready || capabilities.is_empty() {
        return runs_on_unavailable(value, status, p);
    }

    let mut entries: Vec<TagEntry> = capabilities
        .iter()
        .map(|tag| {
            let (key, model, effort) = parse_capability_tag(tag);
            TagEntry { key, model, effort, tag: tag.clone(), announced: true }
        })
        .collect();
    // Pin an off-registry stored value so it stays selectable; every option it
    // adds carries "(offline)" so an edit never silently rewrites the executor.
    if !value.is_empty() && !capabilities.iter().any(|tag| tag == value) {
        let (key, model, effort) = parse_capability_tag(value);
        entries.push(TagEntry { key, model, effort, tag: value.to_owned(), announced: false });
    }

    let current = (!value.is_empty()).then(|| parse_capability_tag(value));
    let group_key = current.as_ref().map_or(String::new(), |tag| tag.0.clone());
    let model_key = current.as_ref().and_then(|tag| tag.1.clone()).unwrap_or_default();
    let current_effort = current.as_ref().and_then(|tag| tag.2.clone());
    let group: Vec<TagEntry> = entries
        .iter()
        .filter(|entry| entry.key == group_key)
        .cloned()
        .collect();

    let offline_mark = |offline: bool, label: &str| {
        if offline {
            format!("{label} (offline)")
        } else {
            label.to_owned()
        }
    };

    // Provider select.
    let mut provider_options: Vec<PickOption> = Vec::new();
    for entry in &entries {
        if provider_options.iter().any(|option| option.value == entry.key) {
            continue;
        }
        let offline = !entries.iter().any(|other| other.key == entry.key && other.announced);
        provider_options.push(PickOption {
            value: entry.key.clone(),
            label: offline_mark(offline, &title_case(&entry.key)),
        });
    }
    let provider_selected = (!group_key.is_empty())
        .then(|| provider_options.iter().find(|option| option.value == group_key).cloned())
        .flatten();
    let provider_entries = entries.clone();
    let provider = pick_list(
        provider_options,
        provider_selected,
        move |option: PickOption| on_change(pick_provider(&provider_entries, &option.value)),
    )
    .placeholder("Choose an executor…")
    .text_size(BODY)
    .width(Length::Fill);

    // Model select.
    let models: Vec<String> = group
        .iter()
        .filter_map(|entry| entry.model.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut model_options: Vec<PickOption> = Vec::new();
    if group.iter().any(|entry| entry.model.is_none()) || models.is_empty() {
        let offline = !group.is_empty()
            && !group.iter().any(|entry| entry.model.is_none() && entry.announced);
        model_options.push(PickOption { value: String::new(), label: offline_mark(offline, "Default") });
    }
    for model in &models {
        let offline = !group
            .iter()
            .any(|entry| entry.model.as_deref() == Some(model) && entry.announced);
        model_options.push(PickOption { value: model.clone(), label: offline_mark(offline, model) });
    }
    let model_selected = model_options.iter().find(|option| option.value == model_key).cloned();
    let model_group = group.clone();
    let model_effort = current_effort.clone();
    let model_group_key = group_key.clone();
    let model = pick_list(model_options, model_selected, move |option: PickOption| {
        on_change(pick_model(&model_group, &model_group_key, model_effort.as_deref(), &option.value))
    })
    .text_size(BODY)
    .width(Length::Fill);

    // Effort select.
    let effort_options: Vec<PickOption> = if model_key.is_empty() {
        vec![PickOption { value: String::new(), label: "Default".into() }]
    } else {
        group
            .iter()
            .filter(|entry| entry.model.as_deref() == Some(model_key.as_str()))
            .filter_map(|entry| entry.effort.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .map(|effort| {
                let offline = !group.iter().any(|entry| {
                    entry.model.as_deref() == Some(model_key.as_str())
                        && entry.effort.as_deref() == Some(effort.as_str())
                        && entry.announced
                });
                PickOption { value: effort.clone(), label: offline_mark(offline, &effort) }
            })
            .collect()
    };
    let effort_selected = effort_options
        .iter()
        .find(|option| Some(&option.value) == current_effort.as_ref())
        .or_else(|| effort_options.iter().find(|option| option.value.is_empty()))
        .cloned();
    let effort_group = group.clone();
    let effort_model_key = model_key.clone();
    let effort = pick_list(effort_options, effort_selected, move |option: PickOption| {
        on_change(pick_effort(&effort_group, &effort_model_key, &option.value))
    })
    .text_size(BODY)
    .width(Length::Fill);

    let mut picker = column![
        section_label("RUNS ON", p),
        provider,
        row![
            column![section_label("MODEL", p), model].spacing(4).width(Length::Fill),
            column![section_label("EFFORT", p), effort].spacing(4).width(Length::Fill),
        ]
        .spacing(9),
    ]
    .spacing(6)
    .width(Length::Fill);
    if !value.is_empty() {
        picker = picker.push(
            text(value.to_owned())
                .font(MONO)
                .size(CAPTION)
                .color(p.muted_2)
                .width(Length::Fill)
                .wrapping(text::Wrapping::WordOrGlyph),
        );
    }
    picker.into()
}

fn runs_on_unavailable<'a>(
    value: &'a str,
    status: CapabilityStatus,
    p: Colors,
) -> Element<'a, Message> {
    let failed = status == CapabilityStatus::Error;
    let guidance = if failed {
        "Could not load the provider registry. Check the node connection, then retry."
    } else if status == CapabilityStatus::Loading {
        "Loading available LLM providers from the network…"
    } else {
        "No LLM provider is available. Under Node → Sandbox, choose an execution mode, then \
         restart the node. Available providers appear here automatically."
    };
    let mut column = column![
        section_label("RUNS ON", p),
        disabled_note(
            if failed {
                "Providers unavailable"
            } else if status == CapabilityStatus::Loading {
                "Loading providers…"
            } else {
                "No provider available"
            },
            p,
        ),
        text(guidance).font(SANS).size(BODY).color(p.muted_2),
    ]
    .spacing(6)
    .width(Length::Fill);
    if !value.is_empty() {
        column = column.push(
            text(value.to_owned())
                .font(MONO)
                .size(CAPTION)
                .color(p.muted_2)
                .width(Length::Fill)
                .wrapping(text::Wrapping::WordOrGlyph),
        );
    }
    if failed {
        column = column.push(secondary_button("Retry", Some(Message::RetryCapabilities), p));
    }
    column.into()
}

fn pick_provider(entries: &[TagEntry], key: &str) -> String {
    let list: Vec<&TagEntry> = entries.iter().filter(|entry| entry.key == key).collect();
    let first_model = list.first().and_then(|entry| entry.model.clone());
    list.iter()
        .find(|entry| entry.model.is_none())
        .or_else(|| {
            list.iter()
                .find(|entry| entry.model == first_model && entry.effort.as_deref() == Some("medium"))
        })
        .or_else(|| list.first())
        .map_or_else(|| key.to_owned(), |entry| entry.tag.clone())
}

fn pick_model(group: &[TagEntry], group_key: &str, effort: Option<&str>, model: &str) -> String {
    if model.is_empty() {
        return group
            .iter()
            .find(|entry| entry.model.is_none())
            .map_or_else(|| group_key.to_owned(), |entry| entry.tag.clone());
    }
    let list: Vec<&TagEntry> = group
        .iter()
        .filter(|entry| entry.model.as_deref() == Some(model))
        .collect();
    list.iter()
        .find(|entry| entry.effort.as_deref() == effort)
        .or_else(|| list.iter().find(|entry| entry.effort.as_deref() == Some("medium")))
        .or_else(|| list.first())
        .map_or_else(|| model.to_owned(), |entry| entry.tag.clone())
}

fn pick_effort(group: &[TagEntry], model_key: &str, effort: &str) -> String {
    group
        .iter()
        .find(|entry| {
            entry.model.as_deref() == Some(model_key) && entry.effort.as_deref() == Some(effort)
        })
        .map_or_else(|| effort.to_owned(), |entry| entry.tag.clone())
}

fn labeled_input<'a>(
    label: &'static str,
    placeholder: &'a str,
    value: &'a str,
    on_input: fn(String) -> Message,
    p: Colors,
) -> Element<'a, Message> {
    column![
        section_label(label, p),
        sem_input(
            label,
            value,
            ui::input::input(placeholder, value, &tokens(p)).on_input(on_input),
        )
    ]
    .spacing(5)
    .width(Length::Fill)
    .into()
}

fn labeled_mono_input<'a>(
    label: &'static str,
    placeholder: &'a str,
    value: &'a str,
    on_input: fn(String) -> Message,
    p: Colors,
) -> Element<'a, Message> {
    column![
        section_label(label, p),
        sem_input(
            label,
            value,
            ui::input::input(placeholder, value, &tokens(p))
                .font(MONO)
                .on_input(on_input),
        )
    ]
    .spacing(5)
    .width(Length::Fill)
    .into()
}

fn info_row(label: &'static str, value: &str, p: Colors) -> Element<'static, Message> {
    container(
        row![
            text(label).font(MONO).size(LABEL).color(p.muted_2),
            Space::new().width(14),
            // A spaceless id/address (`id@agents.duck`) is one unbreakable word;
            // WordOrGlyph + width(Fill) wraps it instead of overflowing the pane.
            text(value.to_owned())
                .font(MONO)
                .size(LABEL)
                .color(p.muted_3)
                .width(Length::Fill)
                .wrapping(text::Wrapping::WordOrGlyph)
        ]
        .spacing(0)
        .align_y(Alignment::Center),
    )
    .padding([9, 11])
    .style(move |_| rounded_surface(p.paper, p.border, RADIUS_SM))
    .into()
}

fn stat(label: &'static str, value: String, p: Colors) -> Element<'static, Message> {
    column![
        section_label(label, p),
        text(value).font(MONO).size(TITLE).color(p.ink)
    ]
    .spacing(5)
    .width(Length::Fill)
    .into()
}

fn avatar(
    name: &str,
    size: f32,
    background: Color,
    foreground: Color,
    p: Colors,
) -> Element<'static, Message> {
    container(
        text(initials(name))
            .font(MONO)
            .size((size * 0.31).max(10.0))
            .color(foreground),
    )
    .width(size)
    .height(size)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(move |_| {
        rounded_surface(
            background,
            mix(background, p.paper, 0.16),
            (size * 0.24).max(7.0),
        )
    })
    .into()
}

fn icon_tile(icon: Icon, size: f32, p: Colors) -> Element<'static, Message> {
    container(icons::view(icon, size * 0.53, p.on_filled))
        .width(size)
        .height(size)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |_| rounded_surface(p.filled, Color::TRANSPARENT, RADIUS_SM))
        .into()
}

fn status_dot(color: Color) -> Element<'static, Message> {
    container(Space::new())
        .width(6)
        .height(6)
        .style(move |_| rounded_surface(color, Color::TRANSPARENT, 99.0))
        .into()
}

fn section_label(label: &str, p: Colors) -> Element<'static, Message> {
    text(label.to_owned())
        .font(MONO)
        .size(CAPTION)
        .color(p.muted_2)
        .into()
}

fn pill(label: &str, tone: Color, p: Colors) -> Element<'static, Message> {
    container(text(label.to_owned()).font(MONO).size(CAPTION).color(tone))
        .padding([3, 7])
        .style(move |_| rounded_surface(mix(p.paper, tone, 0.09), mix(p.paper, tone, 0.25), 5.0))
        .into()
}

fn on_dark_pill(label: &str, tone: Color, p: Colors) -> Element<'static, Message> {
    container(
        text(label.to_owned())
            .font(MONO)
            .size(CAPTION)
            .color(mix(p.on_filled, tone, 0.35)),
    )
    .padding([3, 7])
    .style(move |_| {
        rounded_surface(
            mix(p.filled, p.on_filled, 0.08),
            mix(p.filled, p.on_filled, 0.16),
            999.0,
        )
    })
    .into()
}

fn card<'a>(content: impl Into<Element<'a, Message>>, p: Colors) -> Element<'a, Message> {
    // Callers pad their own content, so keep the toolkit Card surface padding-less.
    ui::surface::surface(content, ui::surface::SurfaceVariant::Card, &tokens(p))
        .width(Length::Fill)
        .into()
}

fn card_style(p: Colors) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.paper)),
        border: Border {
            color: p.border,
            width: 1.0,
            radius: RADIUS_LG.into(),
        },
        shadow: Shadow {
            color: Color { a: 0.06, ..p.shadow },
            offset: Vector::new(0.0, 1.0),
            blur_radius: 3.0,
        },
        ..Default::default()
    }
}

fn empty_state(title: &str, detail: &str, p: Colors) -> Element<'static, Message> {
    ui::empty_state::empty_state(
        Some(icon_tile(Icon::Agent, 36.0, p)),
        title.to_owned(),
        detail.to_owned(),
        &tokens(p),
    )
    .into()
}

fn center_state<'a>(
    title: &str,
    detail: &'a str,
    retry: Option<Message>,
    p: Colors,
) -> Element<'a, Message> {
    let mut content = column![
        icon_tile(Icon::Agent, 46.0, p),
        text(title.to_owned())
            .font(SANS_SEMIBOLD)
            .size(TITLE)
            .color(p.ink)
    ]
    .spacing(10)
    .align_x(Alignment::Center);
    if !detail.is_empty() {
        content = content.push(selectable_line(detail, p.muted_2, p));
    }
    if retry.is_some() {
        content = content.push(primary_button("Retry", retry, p));
    }
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
}

fn error_banner<'a>(error: &'a str, p: Colors) -> Element<'a, Message> {
    // Keep the selectable inner line: an operator copies error/hash text out of it.
    ui::alert::alert(
        selectable_line(error, p.red, p),
        ui::alert::AlertVariant::Destructive,
        &tokens(p),
    )
    .into()
}

/// A read-only, selectable line — an error/hash/id/address the operator can
/// copy. A `text` widget cannot be selected; a caret-less read-only `text_input`
/// stays focusable and copyable (the `workspace.rs` idiom).
fn selectable_line<'a>(value: &'a str, ink: Color, p: Colors) -> Element<'a, Message> {
    text_input("", value)
        .font(SANS)
        .size(BODY)
        .padding(0)
        .style(move |_, _| iced::widget::text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: ink,
            placeholder: ink,
            value: ink,
            selection: p.accent,
        })
        .into()
}

/// A read-only, selectable mono value — hashes, ids, addresses.
fn selectable_mono<'a>(value: &'a str, ink: Color, p: Colors) -> Element<'a, Message> {
    text_input("", value)
        .font(MONO)
        .size(LABEL)
        .padding(0)
        .style(move |_, _| iced::widget::text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: ink,
            placeholder: ink,
            value: ink,
            selection: p.accent,
        })
        .into()
}

fn secondary_button(
    label: &'static str,
    message: Option<Message>,
    p: Colors,
) -> Element<'static, Message> {
    let enabled = message.is_some();
    let mut builder = ui::button::button(label, &tokens(p))
        .variant(ui::button::ButtonVariant::Outline)
        .size(ui::button::ButtonSize::Small)
        .disabled(!enabled);
    if let Some(message) = message {
        builder = builder.on_press(message);
    }
    let btn = builder.into_widget();
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, btn)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn run_link_button(
    label: String,
    message: Message,
    tone: Color,
    p: Colors,
) -> Element<'static, Message> {
    let _name = label.clone();
    let btn = button(text(label).font(MONO).size(LABEL))
        .padding([2, 7])
        .style(move |_, status| button::Style {
            background: Some(Background::Color(
                if matches!(status, button::Status::Hovered) {
                    p.sunken
                } else {
                    p.paper
                },
            )),
            text_color: tone,
            border: Border {
                color: p.border_strong,
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        })
        .on_press(message);
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, _name, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn primary_button(
    label: &'static str,
    message: Option<Message>,
    p: Colors,
) -> Element<'static, Message> {
    let enabled = message.is_some();
    let mut builder = ui::button::button(label, &tokens(p)).disabled(!enabled);
    if let Some(message) = message {
        builder = builder.on_press(message);
    }
    let btn = builder.into_widget();
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, btn)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn on_dark_button(
    label: &'static str,
    message: Option<Message>,
    p: Colors,
) -> Element<'static, Message> {
    let enabled = message.is_some();
    let btn = button(text(label).font(SANS_SEMIBOLD).size(BODY))
        .padding([7, 12])
        .style(move |_, _| button::Style {
            background: Some(Background::Color(mix(p.filled, p.on_filled, 0.07))),
            text_color: if enabled {
                p.on_filled
            } else {
                mix(p.filled, p.on_filled, 0.45)
            },
            border: Border {
                color: mix(p.filled, p.on_filled, if enabled { 0.22 } else { 0.12 }),
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        })
        .on_press_maybe(message);
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, btn)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn surface(color: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(color)),
        ..Default::default()
    }
}

fn rounded_surface(background: Color, border: Color, radius: f32) -> container::Style {
    container::Style {
        background: Some(Background::Color(background)),
        border: Border {
            color: border,
            width: (border != Color::TRANSPARENT) as u8 as f32,
            radius: radius.into(),
        },
        ..Default::default()
    }
}

fn bottom_rule(background: Color, border: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(background)),
        border: Border {
            color: border,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

fn horizontal_divider(p: Colors) -> Element<'static, Message> {
    ui::separator::horizontal(&tokens(p)).into()
}

fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    match words.as_slice() {
        [] => "AI".into(),
        [one] => one.chars().take(2).collect::<String>().to_uppercase(),
        many => format!(
            "{}{}",
            many[0].chars().next().unwrap_or('A'),
            many.last()
                .and_then(|word| word.chars().next())
                .unwrap_or('I')
        )
        .to_uppercase(),
    }
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    chars.next().map_or_else(String::new, |head| {
        format!("{}{}", head.to_uppercase(), chars.as_str())
    })
}

fn mix(base: Color, tint: Color, amount: f32) -> Color {
    Color {
        r: base.r + (tint.r - base.r) * amount,
        g: base.g + (tint.g - base.g) * amount,
        b: base.b + (tint.b - base.b) * amount,
        a: 1.0,
    }
}
