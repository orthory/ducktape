//! Rendering for the native Agents screen.

use std::ops::Deref;

use iced::widget::{
    Space, button, checkbox, column, container, row, scrollable, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::icons::{self, Icon};
use crate::theme::{self, MONO, Palette, RADIUS_LG, RADIUS_MD, RADIUS_SM, SANS, SANS_SEMIBOLD};

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
        Resource::Loading => center_state("Loading agents...", "", p),
        Resource::Empty => center_state("No agent data", "The workspace has not loaded yet.", p),
        Resource::Error(error) => center_state("Agents unavailable", error, p),
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
    let runs = data.map_or(0, |data| data.pending_runs.len() + data.recent_runs.len());
    container(
        row![
            icon_tile(Icon::Agent, 30.0, p),
            text("Agents").font(SANS_SEMIBOLD).size(18).color(p.ink),
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
    let mut list = column![
        row![
            section_label("ROSTER", p),
            Space::new().width(Length::Fill),
            text(format!("{} total", data.agents.len()))
                .font(MONO)
                .size(10.5)
                .color(p.muted_2)
        ]
        .padding([14, 14])
        .align_y(Alignment::Center)
    ];
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
    container(scrollable(list))
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
    let select = button(
        row![
            avatar(&agent.display_name, 36.0, p.filled, p.on_filled, p),
            column![
                text(agent.display_name.clone())
                    .font(SANS_SEMIBOLD)
                    .size(13.5)
                    .color(if selected { p.accent } else { p.ink }),
                row![
                    status_dot(if active { p.green } else { p.amber }),
                    text(if active { "Active" } else { "Paused" })
                        .font(SANS)
                        .size(10.5)
                        .color(p.muted_3),
                    text("·").color(p.icon_idle),
                    text(capability_short(&agent.capability))
                        .font(MONO)
                        .size(10.5)
                        .color(p.muted_2)
                ]
                .spacing(6)
                .align_y(Alignment::Center)
            ]
            .spacing(3)
            .width(Length::Fill)
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([12, 14])
    .style(move |_, status| button::Style {
        background: (selected || matches!(status, button::Status::Hovered))
            .then_some(Background::Color(if selected { p.sunken } else { p.hover })),
        text_color: p.ink,
        border: Border {
            color: p.border_soft,
            width: 1.0,
            radius: 0.0.into(),
        },
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
                        text("Add an agent").font(SANS_SEMIBOLD).size(13.5).color(p.ink),
                        text("Give it a name, pick what it runs on, and curate the documents it carries.")
                            .font(SANS)
                            .size(11.5)
                            .color(p.muted_2)
                    ]
                    .spacing(3)
                    .width(Length::Fill),
                    pill("AGENT", p.accent, p)
                ]
                .spacing(12)
                .align_y(Alignment::Start),
                row![
                    labeled_input(
                        "AGENT DISPLAY NAME",
                        "Triage Agent…",
                        &draft.display_name,
                        Message::RegisterNameChanged,
                        p,
                    ),
                    labeled_input(
                        "RUNS ON",
                        "codex_gpt-5_medium",
                        &draft.capability,
                        Message::RegisterCapabilityChanged,
                        p,
                    )
                ]
                .spacing(9),
                skill_editor(state, p),
                permission_grid(&draft.allowed_actions, false, p),
                section_label("RESOURCE CAPS", p),
                checkbox(draft.library_read)
                    .label("Can search the global skill library")
                    .on_toggle(Message::RegisterLibraryChanged)
                    .size(15)
                    .text_size(10.5),
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
                    .size(10.5)
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
                    text(format!("Agent id: {}", draft.id())).font(MONO).size(10.5).color(p.muted_2).into()
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
    if data.capability_status == CapabilityStatus::Error {
        form = form.push(
            row![
                text("Could not load executor capabilities.")
                    .font(SANS)
                    .size(11.5)
                    .color(p.red),
                secondary_button("Retry", Some(Message::RetryCapabilities), p)
            ]
            .spacing(8),
        );
    }
    if let Some(error) = &state.error {
        form = form.push(error_banner(error, p));
    }
    form.into()
}

fn skill_editor(state: &State, p: Colors) -> Element<'_, Message> {
    let draft = &state.register;
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
                    .size(12)
                    .color(p.ink),
                text(skill.source_prefix.clone())
                    .font(MONO)
                    .size(10.5)
                    .color(p.muted_2)
                    .width(Length::Fill),
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
                text_input("Skill name", &draft.skill_name)
                    .font(SANS)
                    .size(11.5)
                    .on_input(Message::SkillNameChanged),
            ),
            sem_input(
                "Skill prefix",
                &draft.skill_prefix,
                text_input("/skills/name", &draft.skill_prefix)
                    .font(MONO)
                    .size(11.5)
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
                    .size(20)
                    .color(p.on_filled),
                row![
                    text(agent.id.clone()).font(MONO).size(11).color(mix(
                        p.filled,
                        p.on_filled,
                        0.7
                    )),
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
        section_label("CURATED SKILLS", p)
    ]
    .spacing(8)
    .padding(18);
    if agent.skills.is_empty() {
        body = body.push(
            text("No curated skills.")
                .font(SANS)
                .size(11.5)
                .color(p.muted_2),
        );
    } else {
        for skill in &agent.skills {
            body = body.push(skill_row(skill, p));
        }
    }
    body = body
        .push(section_label("IDENTITY", p))
        .push(info_row(
            "Agent address",
            &format!("{}@agents.duck", agent.id),
            p,
        ))
        .push(info_row("Owner", &agent.owner.label(), p))
        .push(info_row("Created", &agent.created_at, p))
        .push(info_row("Updated", &agent.updated_at, p))
        .push(section_label("PERMISSIONS", p));
    if agent.allowed_actions.is_empty() {
        body = body.push(
            text("Can't take any actions yet.")
                .font(SANS)
                .size(11.5)
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
    let valid = !edit.display_name.trim().is_empty()
        && data.capabilities.contains(&edit.capability)
        && !edit.allowed_actions.is_empty()
        && !state.busy;
    let form = column![
        horizontal_divider(p),
        section_label("EDIT AGENT", p),
        row![
            labeled_input(
                "DISPLAY NAME",
                "Name",
                &edit.display_name,
                Message::EditNameChanged,
                p
            ),
            labeled_input(
                "RUNS ON",
                "Capability",
                &edit.capability,
                Message::EditCapabilityChanged,
                p
            )
        ]
        .spacing(9),
        permission_grid(&edit.allowed_actions, true, p),
        section_label("RESOURCE CAPS", p),
        checkbox(edit.library_read)
            .label("Can search the global skill library")
            .on_toggle(Message::EditLibraryChanged)
            .size(15)
            .text_size(10.5),
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
            .size(10.5)
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
    let mut grid = column![section_label("PERMISSIONS", p)].spacing(7);
    for (action, label) in ACTIONS {
        let action_owned = action.to_owned();
        let checked = selected.iter().any(|value| value == action);
        grid = grid.push(
            checkbox(checked)
                .label(label)
                .on_toggle(move |_| {
                    if editing {
                        Message::ToggleEditAction(action_owned.clone())
                    } else {
                        Message::ToggleRegisterAction(action_owned.clone())
                    }
                })
                .size(15)
                .text_size(10.5),
        );
    }
    grid.into()
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
                .size(12)
                .color(p.ink),
            text(format!(
                "{}/SKILL.md",
                skill.source_prefix.trim_end_matches('/')
            ))
            .font(MONO)
            .size(10.5)
            .color(p.muted_2)
            .width(Length::Fill)
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
            text("Agent not found").font(SANS_SEMIBOLD).size(16).color(p.ink),
            text(format!("{id} isn’t in this workspace’s roster — it may have been removed since it was mentioned."))
                .font(SANS)
                .size(12)
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
                .size(16)
                .color(p.ink),
            text("Add your first agent to start automating chats and tasks.")
                .font(SANS)
                .size(12)
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
            .size(11.5)
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
        for watch in &data.watches {
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
                text(format!("# {label}")).font(MONO).size(12).color(p.ink),
                text(policy).font(SANS).size(11.5).color(p.muted_2)
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
    .style(move |_| bottom_rule(p.paper, p.border_soft))
    .into()
}

fn watch_form<'a>(state: &'a State, data: &'a AgentData, p: Colors) -> Element<'a, Message> {
    let policy_ready =
        state.watch.policy != WatchPolicyKind::Assigned || !state.watch.assigned_agent.is_empty();
    let mut form = column![
        section_label("ADD A CHANNEL", p),
        labeled_input(
            "CHANNEL",
            data.channels
                .first()
                .map_or("general", |channel| channel.name.as_str()),
            &state.watch.channel_id,
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
        form = form.push(labeled_input(
            "AGENT",
            data.agents
                .first()
                .map_or("agent-id", |agent| agent.id.as_str()),
            &state.watch.assigned_agent,
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
                .size(10)
                .color(p.muted_2)
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    );
    if let Some(error) = &data.recent_runs_error {
        body = body.push(error_banner(
            &format!("Run history unavailable: {error}"),
            p,
        ));
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
    card(
        row![
            icon_tile(Icon::Agent, 34.0, p),
            column![
                text("Jobs worker")
                    .font(SANS_SEMIBOLD)
                    .size(13.5)
                    .color(p.ink),
                text("Let active agents claim work from the jobs board.")
                    .font(SANS)
                    .size(11.5)
                    .color(p.muted_2),
                text("Current committed status is not readable on this network.")
                    .font(SANS)
                    .size(10.5)
                    .color(p.muted_2)
            ]
            .spacing(3)
            .width(Length::Fill),
            secondary_button(
                "Enable",
                (!data.job_worker_pending).then_some(Message::SetJobWorker(true)),
                p,
            ),
            secondary_button(
                "Disable",
                (!data.job_worker_pending).then_some(Message::SetJobWorker(false)),
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
                text("Usage").font(SANS_SEMIBOLD).size(13).color(p.ink),
                Space::new().width(Length::Fill),
                text("No usage yet").font(SANS).size(11.5).color(p.muted_2)
            ]
            .padding([12, 14]),
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
    let mut content = column![
        row![
            avatar(&agent, 34.0, p.filled, p.on_filled, p),
            column![
                row![
                    text(agent).font(SANS_SEMIBOLD).size(13).color(p.ink),
                    pill("RUNNING", p.blue, p)
                ]
                .spacing(8),
                text(if let Some(job) = &run.job_id {
                    format!("job {job} · dispatch {}", short(&run.dispatch_id))
                } else {
                    format!(
                        "#{channel} · message {} · dispatch {}",
                        run.anchor_sequence,
                        short(&run.dispatch_id)
                    )
                })
                .font(MONO)
                .size(10.5)
                .color(p.muted_2),
                text(run.created_at.clone())
                    .font(SANS)
                    .size(10.5)
                    .color(p.muted_2)
            ]
            .spacing(4)
            .width(Length::Fill),
            secondary_button(
                if expanded { "Hide log" } else { "Live log" },
                Some(Message::ToggleRunLog(run.dispatch_id.clone())),
                p,
            ),
            secondary_button(
                "Reassign",
                (!run.pending).then_some(Message::ReassignRun(run.run_id.clone(), run.attempt + 1)),
                p,
            ),
            secondary_button(
                "Cancel",
                (!run.pending).then_some(Message::CancelRun(run.run_id.clone())),
                p,
            )
        ]
        .spacing(11)
        .padding([12, 14])
        .align_y(Alignment::Center)
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
        metadata = metadata.push(text(target).font(MONO).size(10.5).color(p.muted_2));
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
    metadata = metadata
        .push(Space::new().width(Length::Fill))
        .push(secondary_button(
            if expanded { "Hide log" } else { "Log" },
            Some(Message::ToggleRunLog(run.dispatch_id.clone())),
            p,
        ));
    if let Some(reference) = &run.output_ref {
        metadata = metadata.push(text(short(reference)).font(MONO).size(9.5).color(p.muted_2));
    }
    let mut content = column![
        row![
            avatar(&agent, 30.0, p.filled, p.on_filled, p),
            text(agent).font(SANS_SEMIBOLD).size(12.5).color(p.ink)
        ]
        .spacing(9)
        .align_y(Alignment::Center),
        metadata,
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
                .size(10)
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
                    text(label).font(MONO).size(9).color(p.muted_2).width(52),
                    text(row.text).font(MONO).size(10.5).color(color),
                ]
                .spacing(8),
            );
        }
    }
    if log.is_none_or(|log| log.entries.is_empty()) {
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
            .size(11.5)
            .color(p.muted_2),
        );
    }
    container(scrollable(output.spacing(3)))
        .height(Length::Fixed(180.0))
        .padding([8, 10])
        .style(move |_| rounded_surface(p.canvas, p.border_soft, RADIUS_SM))
        .into()
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
            text(label).font(SANS_SEMIBOLD).size(12),
            container(text(count).font(MONO).size(10).color(if tab == active {
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
                color: Color::from_rgba8(0, 0, 0, 0.07),
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
    let btn = button(text(label).font(SANS_SEMIBOLD).size(10.5))
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
    let btn = button(text(label).font(SANS_SEMIBOLD).size(11))
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
            .size(12.5)
            .color(p.accent)
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if let Some(model) = parts.get(1) {
        strip = strip.push(text("›").font(MONO).size(12).color(p.icon_idle));
        strip = strip.push(text((*model).to_owned()).font(MONO).size(11.5).color(p.ink));
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
            text_input(placeholder, value)
                .font(SANS)
                .size(12.5)
                .on_input(on_input),
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
            text_input(placeholder, value)
                .font(MONO)
                .size(12.5)
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
            text(label).font(MONO).size(11).color(p.muted_2),
            Space::new().width(Length::Fill),
            text(value.to_owned()).font(MONO).size(11).color(p.muted_3)
        ]
        .spacing(14),
    )
    .padding([9, 11])
    .style(move |_| rounded_surface(p.paper, p.border, RADIUS_SM))
    .into()
}

fn stat(label: &'static str, value: String, p: Colors) -> Element<'static, Message> {
    column![
        section_label(label, p),
        text(value).font(MONO).size(16).color(p.ink)
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
        .size(9)
        .color(p.muted_2)
        .into()
}

fn pill(label: &str, tone: Color, p: Colors) -> Element<'static, Message> {
    container(text(label.to_owned()).font(MONO).size(9).color(tone))
        .padding([3, 7])
        .style(move |_| rounded_surface(mix(p.paper, tone, 0.09), mix(p.paper, tone, 0.25), 5.0))
        .into()
}

fn on_dark_pill(label: &str, tone: Color, p: Colors) -> Element<'static, Message> {
    container(
        text(label.to_owned())
            .font(MONO)
            .size(9)
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
    container(content)
        .width(Length::Fill)
        .style(move |_| card_style(p))
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
            color: Color::from_rgba8(0, 0, 0, 0.06),
            offset: Vector::new(0.0, 1.0),
            blur_radius: 3.0,
        },
        ..Default::default()
    }
}

fn empty_state(title: &str, detail: &str, p: Colors) -> Element<'static, Message> {
    column![
        icon_tile(Icon::Agent, 36.0, p),
        text(title.to_owned())
            .font(SANS_SEMIBOLD)
            .size(14)
            .color(p.muted_3),
        text(detail.to_owned())
            .font(SANS)
            .size(11.5)
            .color(p.muted_2)
    ]
    .spacing(8)
    .padding([30, 18])
    .align_x(Alignment::Center)
    .into()
}

fn center_state<'a>(title: &str, detail: &str, p: Colors) -> Element<'a, Message> {
    let mut content = column![
        icon_tile(Icon::Agent, 46.0, p),
        text(title.to_owned())
            .font(SANS_SEMIBOLD)
            .size(16)
            .color(p.ink)
    ]
    .spacing(10)
    .align_x(Alignment::Center);
    if !detail.is_empty() {
        content = content.push(text(detail.to_owned()).font(SANS).size(12).color(p.muted_2));
    }
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
}

fn error_banner(error: &str, p: Colors) -> Element<'static, Message> {
    container(text(error.to_owned()).font(SANS).size(11).color(p.red))
        .width(Length::Fill)
        .padding([7, 9])
        .style(move |_| {
            rounded_surface(
                mix(p.paper, p.red, 0.09),
                mix(p.paper, p.red, 0.25),
                RADIUS_SM,
            )
        })
        .into()
}

fn secondary_button(
    label: &'static str,
    message: Option<Message>,
    p: Colors,
) -> Element<'static, Message> {
    let enabled = message.is_some();
    let btn = button(text(label).font(SANS_SEMIBOLD).size(12))
        .padding([7, 12])
        .style(move |_, status| button::Style {
            background: Some(Background::Color(
                if matches!(status, button::Status::Hovered) && enabled {
                    p.sunken
                } else {
                    p.paper
                },
            )),
            text_color: if enabled { p.ink_soft } else { p.muted_2 },
            border: Border {
                color: p.border_strong,
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

fn run_link_button(
    label: String,
    message: Message,
    tone: Color,
    p: Colors,
) -> Element<'static, Message> {
    let _name = label.clone();
    let btn = button(text(label).font(MONO).size(10.5))
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
    let btn = button(text(label).font(SANS_SEMIBOLD).size(12))
        .padding([7, 12])
        .style(move |_, _| button::Style {
            background: Some(Background::Color(if enabled { p.accent } else { p.chip })),
            text_color: if enabled { Color::WHITE } else { p.muted_2 },
            border: Border {
                color: if enabled { p.accent } else { p.chip },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            shadow: if enabled {
                Shadow {
                    color: Color::from_rgba8(160, 90, 60, 0.3),
                    offset: Vector::new(0.0, 1.0),
                    blur_radius: 2.0,
                }
            } else {
                Shadow::default()
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

fn on_dark_button(
    label: &'static str,
    message: Option<Message>,
    p: Colors,
) -> Element<'static, Message> {
    let enabled = message.is_some();
    let btn = button(text(label).font(SANS_SEMIBOLD).size(12))
        .padding([7, 12])
        .style(move |_, _| button::Style {
            background: Some(Background::Color(mix(p.filled, p.on_filled, 0.07))),
            text_color: if enabled {
                p.on_filled
            } else {
                mix(p.filled, p.on_filled, 0.45)
            },
            border: Border {
                color: mix(p.filled, p.on_filled, 0.22),
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
    container(Space::new())
        .height(1)
        .style(move |_| surface(p.border_soft))
        .into()
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
