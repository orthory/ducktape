use super::*;
use iced::widget::{column, row};
fn category_color(category: ModuleCategory, p: Palette) -> Color {
    match category {
        ModuleCategory::Workspace => p.blue,
        ModuleCategory::Developer => p.purple,
        ModuleCategory::Automation => p.amber,
        ModuleCategory::System => p.muted_3,
    }
}

fn module_info(id: &str) -> (&'static str, &'static str) {
    match id {
        "chat" => ("Chat", "Channels, messages, threads, and reactions."),
        "tasks" => ("Tasks", "A shared, ordered task list."),
        "forge" => ("Forge", "A git-backed repository, one commit per block."),
        "agent" => ("Agents", "The agent collaboration loop and run ledger."),
        "governance" => ("Governance", "Validator-set proposals and quorum voting."),
        "vaults" => ("Vaults", "Encrypted team secrets with an owner/reader ACL."),
        "inbox" => ("Inbox", "Per-member notification queues."),
        "automations" => ("Automations", "Event-triggered rules over module events."),
        "files" => ("Files", "A copy-on-write, content-addressed filesystem."),
        "identity" => ("Identity", "Accounts, member keys, and node bindings."),
        "duckdns" => (
            "DuckDNS",
            "Optional global .duck handles resolved to accounts.",
        ),
        "gateway" => ("Gateway", "Signed account routes to DuckFS or local HTTP."),
        _ => ("Module", "A registered genesis module."),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleCategory {
    Workspace,
    Developer,
    Automation,
    System,
}

impl ModuleCategory {
    const ALL: [Self; 4] = [
        Self::Workspace,
        Self::Developer,
        Self::Automation,
        Self::System,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Workspace => "WORKSPACE",
            Self::Developer => "DEVELOPER",
            Self::Automation => "AUTOMATION",
            Self::System => "SYSTEM",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModulesState {
    pub data: Resource<Vec<ModuleRoot>>,
    pub copied: Option<String>,
}

pub(super) fn copy(state: &mut ModulesState, id: String, root: String) -> Option<Command> {
    state.copied = Some(id);
    Some(Command::CopyText(root))
}

pub(super) fn view(state: &ModulesState, p: Palette) -> Element<'_, Message> {
    let Resource::Ready(modules) = &state.data else {
        return resource_screen(
            &state.data,
            "Modules",
            "Waiting for module roots from the node.",
            Screen::Modules,
            Icon::Modules,
            p,
        );
    };
    let header = screen_header("Modules", Some(modules.len()), p);
    let intro = row![
        icon_tile(Icon::Modules, 36.0, p),
        column![
            text("Node module set").font(SANS).size(19).color(p.ink),
            text("These are the genesis modules this node runs, with each module's committed Merkle root.")
                .font(SANS).size(13).color(p.muted),
        ].spacing(3),
    ].spacing(11).align_y(Alignment::Start);
    let mut body = column![intro].spacing(18);
    for category in ModuleCategory::ALL {
        let rows: Vec<_> = modules
            .iter()
            .filter(|module| module.category == category)
            .collect();
        if rows.is_empty() {
            continue;
        }
        let mut group = column![row![
            text(category.label())
                .font(MONO)
                .size(10)
                .color(category_color(category, p)),
            Space::new().width(Length::Fill),
            text(rows.len().to_string())
                .font(MONO)
                .size(11)
                .color(p.muted_2),
        ]]
        .spacing(10);
        for module in rows {
            group = group.push(module_card(
                module,
                state.copied.as_deref() == Some(module.id.as_str()),
                p,
            ));
        }
        body = body.push(group);
    }
    container(column![
        header,
        scrollable(container(body).padding([22, 26]))
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| surface(p.canvas))
    .into()
}

fn module_card(module: &ModuleRoot, copied: bool, p: Palette) -> Element<'static, Message> {
    let (label, detail) = module_info(&module.id);
    let copy_btn = button(
        text(if copied {
            "copied".into()
        } else {
            short(&module.root, 10, 8)
        })
        .font(MONO)
        .size(11),
    )
    .padding([4, 8])
    .style(move |_, _| iced::widget::button::Style {
        background: Some(Background::Color(if copied {
            p.danger_soft
        } else {
            p.sunken
        })),
        text_color: if copied { p.green } else { p.muted_3 },
        border: Border {
            color: p.border_soft,
            width: 1.0,
            radius: RADIUS_SM.into(),
        },
        ..Default::default()
    })
    .on_press(Message::CopyModule {
        id: module.id.clone(),
        root: module.root.clone(),
    });
    #[cfg(all(feature = "agent", debug_assertions))]
    let copy_btn = iced_agent_plugin::sem(iced_agent_plugin::Role::Button, "Copy root", copy_btn);
    card(
        row![
            container(
                text(
                    module
                        .id
                        .chars()
                        .take(2)
                        .collect::<String>()
                        .to_ascii_uppercase()
                )
                .font(MONO)
                .size(13)
                .color(p.on_filled)
            )
            .width(40)
            .height(40)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(move |_| rounded_surface(p.filled, p.filled, 10.0)),
            column![
                row![
                    text(label).font(SANS).size(13.5).color(p.ink),
                    text(module.id.clone()).font(MONO).size(11).color(p.muted_2),
                ]
                .spacing(7),
                text(detail).font(SANS).size(12).color(p.muted),
                copy_btn,
            ]
            .spacing(5),
        ]
        .spacing(13)
        .align_y(Alignment::Start),
        p,
    )
}
